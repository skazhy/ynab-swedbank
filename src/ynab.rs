use std::error::Error;

extern crate serde;
use serde::{Deserialize, Serialize};

use log::{debug, error};

static API_URL: &str = "https://api.youneedabudget.com";
static APP_URL: &str = "https://app.youneedabudget.com";

enum UrlType {
    AppUrl,
    ApiUrl,
}

pub fn parse_i64_string(i: &str) -> Option<i64> {
    i.replace(",", "").parse::<i64>().ok()
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
    pub fn add_amount(self, commission: i64) -> Self {
        YnabTransaction {
            amount: self.amount + commission,
            ..self
        }
    }
}

pub struct YnabClient {
    budget_id: String,
    pub account_id: String,
    client: reqwest::blocking::Client,
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
    pub fn new(budget_id: String, account_id: String, token: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );

        Self {
            budget_id,
            account_id,
            client: reqwest::blocking::Client::builder()
                .default_headers(headers)
                .build()
                .unwrap(),
        }
    }

    fn transactions_uri(&self) -> String {
        format!("{}/v1/budgets/{}/transactions", API_URL, self.budget_id)
    }

    fn account_uri(&self, url_type: UrlType) -> String {
        match url_type {
            UrlType::ApiUrl => format!("{}/v1/budgets/{}/accounts/{}", API_URL, self.budget_id, self.account_id),
            UrlType::AppUrl => format!("{}/{}/accounts/{}", APP_URL, self.budget_id, self.account_id),
        }
    }

    pub fn app_account_uri(&self) -> String {
        self.account_uri(UrlType::AppUrl)
    }

    fn budget_uri(&self) -> String {
        format!("{}/v1/budgets/{}", API_URL, self.budget_id)
    }

    fn get<T: for<'a> Deserialize<'a>>(&self, uri: &str) -> Result<T, reqwest::Error> {
        self.client
            .get(uri)
            .send()
            .and_then(|r| {
                debug!("GET {} -> {:?}", uri, r);
                r.json()
            })
            .map_err(|e| {
                error!("GET {} -> {:?}", uri, e);
                e
            })
    }

    fn post<S: Serialize, D: for<'a> Deserialize<'a>>(&self, body: S, uri: &str) -> Result<D, reqwest::Error> {
        self.client
            .post(uri)
            .json(&body)
            .send()
            .and_then(|r| {
                debug!("POST {} -> {:?}", uri, r);
                r.json()
            })
            .map_err(|e| {
                error!("POST {} -> {:?}", uri, e);
                e
            })
    }

    pub fn post_transactions<T: Serialize>(&self, txns: T) -> Result<PostTransactionsResponseData, Box<dyn Error>> {
        let body = PostTransactionsRequest { transactions: txns };
        let res: PostTransactionsResponse = self.post(body, &self.transactions_uri())?;
        Ok(res.data)
    }

    pub fn get_budget_currency(&self) -> Result<String, Box<dyn Error>> {
        let res: GetBudgetResponse = self.get(&self.budget_uri())?;
        Ok(res.data.budget.currency_format.iso_code)
    }

    pub fn get_acccount_balance(&self) -> Result<i64, Box<dyn Error>> {
        let res: GetAccountResponse = self.get(&self.account_uri(UrlType::ApiUrl))?;
        Ok(res.data.account.balance)
    }
}
