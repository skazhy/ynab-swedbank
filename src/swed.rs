extern crate serde;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub enum EntryType {
    #[serde(rename = "K")]
    Credit,
    #[serde(rename = "D")]
    Debit,
}

#[derive(Debug, Deserialize)]
pub enum RecordType {
    #[serde(rename = "10")]
    StartBalance,
    #[serde(rename = "20")]
    Transaction,
    #[serde(rename = "82")]
    Turnover,
    #[serde(rename = "86")]
    EndBalance,
    #[serde(rename = "900")]
    Interest,
}

#[derive(Debug, Deserialize)]
pub struct SwedbankCsv {
    #[serde(rename = "Ieraksta tips")] // 1
    pub record_type: RecordType,
    #[serde(rename = "Datums")] // 2
    pub date: String,
    #[serde(rename = "Saņēmējs/Maksātājs")] // 3
    pub payee: String,
    #[serde(rename = "Informācija saņēmējam")] // 4
    pub memo: String,
    #[serde(rename = "Summa")] // 5
    pub amount: String,
    #[serde(rename = "Debets/Kredīts")] // 7
    pub debit_or_credit: EntryType,
    #[serde(rename = "Arhīva kods")] // 7
    pub transaction_id: String,
    #[serde(rename = "Maksājuma veids")] // 9
    pub payment_type: String,
}
