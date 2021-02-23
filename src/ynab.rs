use std::error::Error;

extern crate serde;
use serde::{Deserialize, Serialize};

pub fn parse_i64_string(i: &str) -> Option<i64> {
    i64::from_str_radix(&i.replace(",", ""), 10).ok()
}

fn no_rollup() -> bool {
    false
}

#[derive(Deserialize, Serialize)]
pub struct YnabTransaction {
    pub import_id: String,
    pub date: String,
    pub payee_name: String,
    pub memo: Option<String>,
    pub cleared: String,
    pub amount: i64,
    pub account_id: String,

    #[serde(skip, default = "no_rollup")]
    pub needs_rollup: bool,
}

impl YnabTransaction {
    pub fn add_amount(self: Self, commission: i64) -> Self {
        YnabTransaction {
            amount: self.amount + commission,
            ..self
        }
    }
}

pub struct YnabClient {
    pub budget_id: String,
    pub account_id: String,
    pub token: String,
}

#[derive(Deserialize)]
struct YnabCurrencyFormat {
    iso_code: String,
}

#[derive(Deserialize)]
struct YnabBudget {
    pub currency_format: YnabCurrencyFormat,
}

#[derive(Deserialize)]
struct YnabAccount {
    balance: i64,
}

#[derive(Deserialize)]
struct GetAccountResponseData {
    account: YnabAccount,
}

#[derive(Deserialize)]
struct GetAccountResponse {
    data: GetAccountResponseData,
}

#[derive(Deserialize)]
struct GetBudgetResponseData {
    budget: YnabBudget,
}

#[derive(Deserialize)]
struct GetBudgetResponse {
    data: GetBudgetResponseData,
}

#[derive(Deserialize)]
pub struct PostTransactionsResponseData {
    // server_knowledge: i64,
    pub duplicate_import_ids: Vec<String>,
    pub transactions: Vec<YnabTransaction>,
}

#[derive(Deserialize)]
struct PostTransactionsResponse {
    data: PostTransactionsResponseData,
}

#[derive(Serialize)]
struct PostTransactionsRequest<T> {
    transactions: T,
}

impl YnabClient {
    fn transactions_uri(self: &Self) -> String {
        format!(
            "https://api.youneedabudget.com/v1/budgets/{}/transactions",
            self.budget_id
        )
    }

    fn account_uri(self: &Self) -> String {
        format!(
            "https://api.youneedabudget.com/v1/budgets/{}/accounts/{}",
            self.budget_id, self.account_id
        )
    }

    fn budget_uri(self: &Self) -> String {
        format!("https://api.youneedabudget.com/v1/budgets/{}", self.budget_id)
    }

    fn get(self: &Self, uri: &str) -> Result<reqwest::blocking::Response, reqwest::Error> {
        let client = reqwest::blocking::Client::new();
        client.get(uri).bearer_auth(&self.token).send()
    }

    fn post<T: Serialize>(self: &Self, body: T, uri: &str) -> Result<reqwest::blocking::Response, reqwest::Error> {
        let client = reqwest::blocking::Client::new();
        client.post(uri).bearer_auth(&self.token).json(&body).send()
    }

    pub fn post_transactions<T: Serialize>(
        self: &Self,
        txns: T,
    ) -> Result<PostTransactionsResponseData, Box<dyn Error>> {
        let body = PostTransactionsRequest { transactions: txns };
        let res: PostTransactionsResponse = self.post(body, &self.transactions_uri())?.json()?;
        Ok(res.data)
    }

    pub fn get_budget_currency(self: &Self) -> Result<String, Box<dyn Error>> {
        let res: GetBudgetResponse = self.get(&self.budget_uri())?.json()?;
        Ok(res.data.budget.currency_format.iso_code)
    }

    pub fn get_acccount_balance(self: &Self) -> Result<i64, Box<dyn Error>> {
        let res: GetAccountResponse = self.get(&self.account_uri())?.json()?;
        Ok(res.data.account.balance)
    }
}
