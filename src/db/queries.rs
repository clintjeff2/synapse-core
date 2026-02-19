use crate::db::models::Transaction;
use crate::db::pool_manager::{PoolManager, QueryIntent};
use sqlx::Result;

pub async fn insert_transaction(pool_manager: &PoolManager, tx: &Transaction) -> Result<Transaction> {
    let pool = pool_manager.get_pool(QueryIntent::Write);
    sqlx::query_as!(
        Transaction,
        "INSERT INTO transactions (id, amount, created_at) VALUES ($1, $2, $3) RETURNING *",
        tx.id,
        tx.amount,
        tx.created_at
    )
    .fetch_one(pool)
    .await
}

pub async fn get_transaction(pool_manager: &PoolManager, id: i32) -> Result<Transaction> {
    let pool = pool_manager.get_pool(QueryIntent::Read);
    sqlx::query_as!(Transaction, "SELECT * FROM transactions WHERE id = $1", id)
        .fetch_one(pool)
        .await
}

pub async fn list_transactions(pool_manager: &PoolManager, limit: i64, offset: i64) -> Result<Vec<Transaction>> {
    let pool = pool_manager.get_pool(QueryIntent::Read);
    sqlx::query_as!(
        Transaction,
        "SELECT * FROM transactions ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        limit,
        offset
    )
    .fetch_all(pool)
    .await
}
