// Client wrapper for UniFFI - exposes torii_client functionality

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use super::types::*;

// Static tokio runtime for all async operations
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        #[cfg(not(target_arch = "wasm32"))]
        let rt = Runtime::new();

        #[cfg(target_arch = "wasm32")]
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build();

        rt.expect("Failed to create tokio runtime")
    })
}

// Callback traits for subscriptions
pub trait EntityUpdateCallback: Send + Sync {
    fn on_update(&self, entity: Entity);
    fn on_error(&self, error: String);
}

pub trait TokenBalanceUpdateCallback: Send + Sync {
    fn on_update(&self, balance: TokenBalance);
    fn on_error(&self, error: String);
}

pub trait TokenUpdateCallback: Send + Sync {
    fn on_update(&self, token: Token);
    fn on_error(&self, error: String);
}

pub trait TransactionUpdateCallback: Send + Sync {
    fn on_update(&self, transaction: Transaction);
    fn on_error(&self, error: String);
}

pub trait EventUpdateCallback: Send + Sync {
    fn on_update(&self, event: Event);
    fn on_error(&self, error: String);
}

/// Main Dojo client for interacting with the Torii indexer
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct ToriiClient {
    inner: Arc<torii_client::Client>,
    subscriptions: Arc<Mutex<HashMap<u64, JoinHandle<()>>>>,
    next_sub_id: Arc<AtomicU64>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl ToriiClient {
    /// Create a new Torii client with default configuration (4MB max message size)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(torii_url: String) -> Result<ToriiClient, DojoError> {
        let client = runtime()
            .block_on(torii_client::Client::new(torii_url))
            .map_err(|_e| DojoError::ConnectionError)?;

        Ok(Self {
            inner: Arc::new(client),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_sub_id: Arc::new(AtomicU64::new(0)),
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn create(torii_url: String) -> Result<ToriiClient, JsValue> {
        let client = torii_client::Client::new(torii_url).await
            .map_err(|_e| DojoError::ConnectionError)?;

        Ok(Self {
            inner: Arc::new(client),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_sub_id: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Create a new Torii client with custom max message size
    pub fn new_with_config(torii_url: String, max_message_size: u64) -> Result<ToriiClient, DojoError> {
        let client = runtime()
            .block_on(torii_client::Client::new_with_config(torii_url, max_message_size as usize))
            .map_err(|_e| DojoError::ConnectionError)?;

        Ok(Self {
            inner: Arc::new(client),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_sub_id: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Publish an offchain message to the world
    /// Returns the entity ID of the published message
    #[cfg(not(target_arch = "wasm32"))]
    pub fn publish_message(&self, message: Message) -> Result<String, DojoError> {
        let msg: torii_proto::Message = message.into();
        let inner = self.inner.clone();
        runtime().block_on(inner.publish_message(msg)).map_err(|_| DojoError::PublishError)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn publish_message(&self, message: JsValue) -> Result<String, JsValue> {
        let message: Message = serde_wasm_bindgen::from_value(message)?;
        let msg: torii_proto::Message = message.into();
        let inner = self.inner.clone();
        inner.publish_message(msg).await.map_err(|_| DojoError::PublishError.into())
    }

    /// Publish multiple offchain messages to the world
    /// Returns the entity IDs of the published messages
    #[cfg(not(target_arch = "wasm32"))]
    pub fn publish_message_batch(&self, messages: Vec<Message>) -> Result<Vec<String>, DojoError> {
        let msgs: Vec<torii_proto::Message> = messages.into_iter().map(|m| m.into()).collect();
        let inner = self.inner.clone();
        runtime().block_on(inner.publish_message_batch(msgs)).map_err(|_| DojoError::PublishError)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn publish_message_batch(&self, messages: JsValue) -> Result<JsValue, JsValue> {
        let messages: Vec<Message> = serde_wasm_bindgen::from_value(messages)?;
        let msgs: Vec<torii_proto::Message> = messages.into_iter().map(|m| m.into()).collect();
        let inner = self.inner.clone();
        let res = inner.publish_message_batch(msgs).await.map_err(|_| DojoError::PublishError)?;
        Ok(serde_wasm_bindgen::to_value(&res)?)
    }

    /// Get world metadata for specified world addresses
    #[cfg(not(target_arch = "wasm32"))]
    pub fn worlds(&self, world_addresses: Vec<FieldElement>) -> Result<Vec<World>, DojoError> {
        let addrs: Result<Vec<starknet::core::types::Felt>, DojoError> =
            world_addresses.iter().map(field_element_to_felt).collect();
        let addrs = addrs?;

        let inner = self.inner.clone();
        let worlds = runtime()
            .block_on(inner.worlds(addrs))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(worlds.into_iter().map(|w| w.into()).collect())
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn worlds(&self, world_addresses: JsValue) -> Result<JsValue, JsValue> {
        let world_addresses: Vec<FieldElement> = serde_wasm_bindgen::from_value(world_addresses)?;
        let addrs: Result<Vec<starknet::core::types::Felt>, DojoError> =
            world_addresses.iter().map(field_element_to_felt).collect();
        let addrs = addrs.map_err(|e| JsValue::from(e.to_string()))?;

        let inner = self.inner.clone();
        let worlds = inner.worlds(addrs).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let worlds: Vec<World> = worlds.into_iter().map(|w| w.into()).collect();
        Ok(serde_wasm_bindgen::to_value(&worlds)?)
    }

    /// Retrieve controllers matching the query
    #[cfg(not(target_arch = "wasm32"))]
    pub fn controllers(&self, query: ControllerQuery) -> Result<PageController, DojoError> {
        let q: torii_proto::ControllerQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.controllers(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageController {
            items: page.items.into_iter().map(|c| c.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn controllers(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: ControllerQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::ControllerQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.controllers(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageController {
            items: page.items.into_iter().map(|c| c.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve contracts matching the query
    #[cfg(not(target_arch = "wasm32"))]
    pub fn contracts(&self, query: ContractQuery) -> Result<Vec<Contract>, DojoError> {
        let q: torii_proto::ContractQuery = query.into();
        let inner = self.inner.clone();
        let contracts = runtime()
            .block_on(inner.contracts(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(contracts.into_iter().map(|c| c.into()).collect())
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn contracts(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: ContractQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::ContractQuery = query.into();
        let inner = self.inner.clone();
        let contracts = inner.contracts(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let contracts: Vec<Contract> = contracts.into_iter().map(|c| c.into()).collect();
        Ok(serde_wasm_bindgen::to_value(&contracts)?)
    }

    /// Retrieve tokens matching the query
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tokens(&self, query: TokenQuery) -> Result<PageToken, DojoError> {
        let q: torii_proto::TokenQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.tokens(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageToken {
            items: page.items.into_iter().map(|t| t.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn tokens(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: TokenQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::TokenQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.tokens(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageToken {
            items: page.items.into_iter().map(|t| t.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve token balances
    #[cfg(not(target_arch = "wasm32"))]
    pub fn token_balances(&self, query: TokenBalanceQuery) -> Result<PageTokenBalance, DojoError> {
        let q: torii_proto::TokenBalanceQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.token_balances(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageTokenBalance {
            items: page.items.into_iter().map(|b| b.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn token_balances(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: TokenBalanceQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::TokenBalanceQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.token_balances(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageTokenBalance {
            items: page.items.into_iter().map(|b| b.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve token contracts
    #[cfg(not(target_arch = "wasm32"))]
    pub fn token_contracts(
        &self,
        query: TokenContractQuery,
    ) -> Result<PageTokenContract, DojoError> {
        let q: torii_proto::TokenContractQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.token_contracts(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageTokenContract {
            items: page.items.into_iter().map(|tc| tc.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn token_contracts(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: TokenContractQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::TokenContractQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.token_contracts(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageTokenContract {
            items: page.items.into_iter().map(|tc| tc.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve token transfers
    #[cfg(not(target_arch = "wasm32"))]
    pub fn token_transfers(
        &self,
        query: TokenTransferQuery,
    ) -> Result<PageTokenTransfer, DojoError> {
        let q: torii_proto::TokenTransferQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.token_transfers(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageTokenTransfer {
            items: page.items.into_iter().map(|t| t.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn token_transfers(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: TokenTransferQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::TokenTransferQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.token_transfers(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageTokenTransfer {
            items: page.items.into_iter().map(|t| t.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve transactions
    #[cfg(not(target_arch = "wasm32"))]
    pub fn transactions(&self, query: TransactionQuery) -> Result<PageTransaction, DojoError> {
        let q: torii_proto::TransactionQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.transactions(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageTransaction {
            items: page.items.into_iter().map(|t| t.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn transactions(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: TransactionQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::TransactionQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.transactions(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageTransaction {
            items: page.items.into_iter().map(|t| t.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve aggregations (leaderboards, stats, rankings)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn aggregations(&self, query: AggregationQuery) -> Result<PageAggregationEntry, DojoError> {
        let q: torii_proto::AggregationQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.aggregations(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageAggregationEntry {
            items: page.items.into_iter().map(|a| a.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn aggregations(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: AggregationQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::AggregationQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.aggregations(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageAggregationEntry {
            items: page.items.into_iter().map(|a| a.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve activities (user session tracking)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn activities(&self, query: ActivityQuery) -> Result<PageActivity, DojoError> {
        let q: torii_proto::ActivityQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.activities(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageActivity {
            items: page.items.into_iter().map(|a| a.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn activities(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: ActivityQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::ActivityQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.activities(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageActivity {
            items: page.items.into_iter().map(|a| a.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve achievements
    #[cfg(not(target_arch = "wasm32"))]
    pub fn achievements(&self, query: AchievementQuery) -> Result<PageAchievement, DojoError> {
        let q: torii_proto::AchievementQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.achievements(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageAchievement {
            items: page.items.into_iter().map(|a| a.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn achievements(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: AchievementQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::AchievementQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.achievements(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageAchievement {
            items: page.items.into_iter().map(|a| a.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve player achievements
    #[cfg(not(target_arch = "wasm32"))]
    pub fn player_achievements(
        &self,
        query: PlayerAchievementQuery,
    ) -> Result<PagePlayerAchievement, DojoError> {
        let q: torii_proto::PlayerAchievementQuery = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.player_achievements(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PagePlayerAchievement {
            items: page.items.into_iter().map(|p| p.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn player_achievements(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: PlayerAchievementQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::PlayerAchievementQuery = query.into();
        let inner = self.inner.clone();
        let page = inner.player_achievements(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PagePlayerAchievement {
            items: page.items.into_iter().map(|p| p.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve entities matching the query
    #[cfg(not(target_arch = "wasm32"))]
    pub fn entities(&self, query: Query) -> Result<PageEntity, DojoError> {
        let q: torii_proto::Query = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.entities(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageEntity {
            items: page.items.into_iter().map(|e| e.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn entities(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: Query = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::Query = query.into();
        let inner = self.inner.clone();
        let page = inner.entities(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageEntity {
            items: page.items.into_iter().map(|e| e.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve event messages matching the query
    #[cfg(not(target_arch = "wasm32"))]
    pub fn event_messages(&self, query: Query) -> Result<PageEntity, DojoError> {
        let q: torii_proto::Query = query.into();
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.event_messages(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageEntity {
            items: page.items.into_iter().map(|e| e.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn event_messages(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: Query = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::Query = query.into();
        let inner = self.inner.clone();
        let page = inner.event_messages(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageEntity {
            items: page.items.into_iter().map(|e| e.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Retrieve raw Starknet events
    #[cfg(not(target_arch = "wasm32"))]
    pub fn starknet_events(&self, query: EventQuery) -> Result<PageEvent, DojoError> {
        let q: torii_proto::EventQuery = query.try_into()?;
        let inner = self.inner.clone();
        let page = runtime()
            .block_on(inner.starknet_events(q))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        Ok(PageEvent {
            items: page.items.into_iter().map(|e| e.into()).collect(),
            next_cursor: page.next_cursor,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn starknet_events(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: EventQuery = serde_wasm_bindgen::from_value(query)?;
        let q: torii_proto::EventQuery = query.try_into().map_err(|e: DojoError| JsValue::from(e.to_string()))?;
        let inner = self.inner.clone();
        let page = inner.starknet_events(q).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let page = PageEvent {
            items: page.items.into_iter().map(|e| e.into()).collect(),
            next_cursor: page.next_cursor,
        };
        Ok(serde_wasm_bindgen::to_value(&page)?)
    }

    /// Execute a SQL query against the Torii database
    #[cfg(not(target_arch = "wasm32"))]
    pub fn sql(&self, query: String) -> Result<Vec<SqlRow>, DojoError> {
        let inner = self.inner.clone();
        let rows = runtime()
            .block_on(inner.sql(query))
            .map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn sql(&self, query: String) -> Result<JsValue, JsValue> {
        let inner = self.inner.clone();
        let rows = inner.sql(query).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;

        let rows: Result<Vec<SqlRow>, DojoError> = rows.into_iter().map(|r| r.try_into()).collect();
        let rows = rows.map_err(|e| JsValue::from(e.to_string()))?;
        Ok(serde_wasm_bindgen::to_value(&rows)?)
    }

    /// Perform a full-text search across indexed entities using FTS5.
    ///
    /// # Arguments
    /// * `query` - Search query containing the search text and limit
    ///
    /// # Returns
    /// A `SearchResponse` containing results grouped by table with relevance scores
    #[cfg(not(target_arch = "wasm32"))]
    pub fn search(&self, query: SearchQuery) -> Result<SearchResponse, DojoError> {
        let inner = self.inner.clone();
        runtime()
            .block_on(inner.search(query.into()))
            .map(Into::into)
            .map_err(|e| DojoError::QueryError { message: e.to_string() })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn search(&self, query: JsValue) -> Result<JsValue, JsValue> {
        let query: SearchQuery = serde_wasm_bindgen::from_value(query)?;
        let inner = self.inner.clone();
        let res = inner.search(query.into()).await.map_err(|e| DojoError::QueryError { message: e.to_string() })?;
        let res: SearchResponse = res.into();
        Ok(serde_wasm_bindgen::to_value(&res)?)
    }

    /// Subscribe to entity updates
    #[cfg(not(target_arch = "wasm32"))]
    pub fn subscribe_entity_updates(
        &self,
        clause: Option<Clause>,
        world_addresses: Vec<FieldElement>,
        callback: Box<dyn EntityUpdateCallback>,
    ) -> Result<u64, DojoError> {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);

        let addrs: Result<Vec<starknet::core::types::Felt>, DojoError> =
            world_addresses.iter().map(field_element_to_felt).collect();
        let addrs = addrs?;

        let clause_proto = clause.map(|c| c.into());

        let inner = self.inner.clone();
        let stream = runtime()
            .block_on(inner.on_entity_updated(clause_proto, addrs))
            .map_err(|_| DojoError::SubscriptionError)?;

        let handle = runtime().spawn(async move {
            use futures_util::StreamExt;
            let mut stream = stream;
            // Skip the first message which contains the subscription ID
            let _ = stream.next().await;

            while let Some(result) = stream.next().await {
                match result {
                    Ok((_id, entity)) => {
                        callback.on_update(entity.into());
                    }
                    Err(e) => {
                        callback.on_error(e.to_string());
                        break;
                    }
                }
            }
        });

        self.subscriptions.lock().unwrap().insert(sub_id, handle);
        Ok(sub_id)
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn subscribe_entity_updates(
        &self,
        clause: JsValue,
        world_addresses: JsValue,
        callback: JsValue,
    ) -> Result<u64, JsValue> {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);

        let world_addresses: Vec<FieldElement> = serde_wasm_bindgen::from_value(world_addresses)?;
        let addrs: Result<Vec<starknet::core::types::Felt>, DojoError> =
            world_addresses.iter().map(field_element_to_felt).collect();
        let addrs = addrs.map_err(|e| JsValue::from(e.to_string()))?;

        let clause: Option<Clause> = serde_wasm_bindgen::from_value(clause)?;
        let clause_proto = clause.map(|c| c.into());

        let inner = self.inner.clone();
        let stream = inner.on_entity_updated(clause_proto, addrs).await
            .map_err(|_| DojoError::SubscriptionError)?;

        let callback_func = js_sys::Function::from(callback);

        wasm_bindgen_futures::spawn_local(async move {
            use futures_util::StreamExt;
            let mut stream = stream;
            // Skip the first message which contains the subscription ID
            let _ = stream.next().await;

            while let Some(result) = stream.next().await {
                match result {
                    Ok((_id, entity)) => {
                        let entity: Entity = entity.into();
                        let entity_js = serde_wasm_bindgen::to_value(&entity).unwrap();

                        // We expect callback to be an object with on_update method or a function
                        // But the JS code passes an object { on_update: ..., on_error: ... }
                        // Let's assume it's an object and try to call on_update

                        let on_update = js_sys::Reflect::get(&callback_func, &JsValue::from_str("on_update"));
                        if let Ok(func) = on_update {
                            if let Ok(func) = func.dyn_into::<js_sys::Function>() {
                                let _ = func.call1(&JsValue::NULL, &entity_js);
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = JsValue::from_str(&e.to_string());
                        let on_error = js_sys::Reflect::get(&callback_func, &JsValue::from_str("on_error"));
                        if let Ok(func) = on_error {
                            if let Ok(func) = func.dyn_into::<js_sys::Function>() {
                                let _ = func.call1(&JsValue::NULL, &error_msg);
                            }
                        }
                        break;
                    }
                }
            }
        });

        // We don't have a JoinHandle in WASM spawn_local, so we can't easily cancel it this way.
        // For now, we just return the ID. Proper cancellation would require an AbortController or similar mechanism.
        Ok(sub_id)
    }

    /// Subscribe to token balance updates
    #[cfg(not(target_arch = "wasm32"))]
    pub fn subscribe_token_balance_updates(
        &self,
        contract_addresses: Vec<FieldElement>,
        account_addresses: Vec<FieldElement>,
        token_ids: Vec<U256>,
        callback: Box<dyn TokenBalanceUpdateCallback>,
    ) -> Result<u64, DojoError> {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);

        let contracts: Result<Vec<starknet::core::types::Felt>, DojoError> =
            contract_addresses.iter().map(field_element_to_felt).collect();
        let accounts: Result<Vec<starknet::core::types::Felt>, DojoError> =
            account_addresses.iter().map(field_element_to_felt).collect();
        let ids: Result<Vec<crypto_bigint::U256>, DojoError> =
            token_ids.iter().map(uniffi_to_u256).collect();

        let inner = self.inner.clone();
        let stream = runtime()
            .block_on(inner.on_token_balance_updated(contracts?, accounts?, ids?))
            .map_err(|_| DojoError::SubscriptionError)?;

        let handle = runtime().spawn(async move {
            use futures_util::StreamExt;
            let mut stream = stream;
            // Skip the first message which contains the subscription ID
            let _ = stream.next().await;

            while let Some(result) = stream.next().await {
                match result {
                    Ok((_id, balance)) => {
                        callback.on_update(balance.into());
                    }
                    Err(e) => {
                        callback.on_error(e.to_string());
                        break;
                    }
                }
            }
        });

        self.subscriptions.lock().unwrap().insert(sub_id, handle);
        Ok(sub_id)
    }

    /// Subscribe to token updates
    #[cfg(not(target_arch = "wasm32"))]
    pub fn subscribe_token_updates(
        &self,
        contract_addresses: Vec<FieldElement>,
        token_ids: Vec<U256>,
        callback: Box<dyn TokenUpdateCallback>,
    ) -> Result<u64, DojoError> {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);

        let contracts: Result<Vec<starknet::core::types::Felt>, DojoError> =
            contract_addresses.iter().map(field_element_to_felt).collect();
        let ids: Result<Vec<crypto_bigint::U256>, DojoError> =
            token_ids.iter().map(uniffi_to_u256).collect();

        let inner = self.inner.clone();
        let stream = runtime()
            .block_on(inner.on_token_updated(contracts?, ids?))
            .map_err(|_| DojoError::SubscriptionError)?;

        let handle = runtime().spawn(async move {
            use futures_util::StreamExt;
            let mut stream = stream;
            // Skip the first message which contains the subscription ID
            let _ = stream.next().await;

            while let Some(result) = stream.next().await {
                match result {
                    Ok((_id, token)) => {
                        callback.on_update(token.into());
                    }
                    Err(e) => {
                        callback.on_error(e.to_string());
                        break;
                    }
                }
            }
        });

        self.subscriptions.lock().unwrap().insert(sub_id, handle);
        Ok(sub_id)
    }

    /// Subscribe to transaction updates
    #[cfg(not(target_arch = "wasm32"))]
    pub fn subscribe_transaction_updates(
        &self,
        filter: Option<TransactionFilter>,
        callback: Box<dyn TransactionUpdateCallback>,
    ) -> Result<u64, DojoError> {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);

        let filter_proto = filter.map(|f| f.into());

        let inner = self.inner.clone();
        let stream = runtime()
            .block_on(inner.on_transaction(filter_proto))
            .map_err(|_| DojoError::SubscriptionError)?;

        let handle = runtime().spawn(async move {
            use futures_util::StreamExt;
            let mut stream = stream;
            // Skip the first message which contains the subscription ID
            let _ = stream.next().await;

            while let Some(result) = stream.next().await {
                match result {
                    Ok(transaction) => {
                        callback.on_update(transaction.into());
                    }
                    Err(e) => {
                        callback.on_error(e.to_string());
                        break;
                    }
                }
            }
        });

        self.subscriptions.lock().unwrap().insert(sub_id, handle);
        Ok(sub_id)
    }

    /// Subscribe to Starknet event updates
    #[cfg(not(target_arch = "wasm32"))]
    pub fn subscribe_event_updates(
        &self,
        keys: Vec<KeysClause>,
        callback: Box<dyn EventUpdateCallback>,
    ) -> Result<u64, DojoError> {
        let sub_id = self.next_sub_id.fetch_add(1, Ordering::SeqCst);

        let keys_proto: Vec<torii_proto::KeysClause> = keys.into_iter().map(|k| k.into()).collect();

        let inner = self.inner.clone();
        let stream = runtime()
            .block_on(inner.on_starknet_event(keys_proto))
            .map_err(|_| DojoError::SubscriptionError)?;

        let handle = runtime().spawn(async move {
            use futures_util::StreamExt;
            let mut stream = stream;
            // Skip the first message which contains the subscription ID
            let _ = stream.next().await;

            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => {
                        callback.on_update(event.into());
                    }
                    Err(e) => {
                        callback.on_error(e.to_string());
                        break;
                    }
                }
            }
        });

        self.subscriptions.lock().unwrap().insert(sub_id, handle);
        Ok(sub_id)
    }

    /// Cancel a subscription
    pub fn cancel_subscription(&self, subscription_id: u64) -> Result<(), DojoError> {
        let mut subs = self.subscriptions.lock().unwrap();
        if let Some(handle) = subs.remove(&subscription_id) {
            handle.abort();
            Ok(())
        } else {
            Err(DojoError::SubscriptionError)
        }
    }
}
