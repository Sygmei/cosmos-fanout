use cosmwasm_std::{Addr, Coin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub only_owner_can_register_beneficiary: bool,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    RegisterBeneficiaryAsOwner { beneficiary: String },
    RegisterBeneficiary {},
    RemoveBeneficiary {},
    RemoveBeneficiaryAsOwner { beneficiary: String },
    AddToPot {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetState {},
    GetDonator { donator: String },
    GetBeneficiary { beneficiary: String },
    GetRemovedBeneficiary { beneficiary: String },
    GetAllDonators {},
    GetAllBeneficiaries {},
    GetAllRemovedBeneficiaries {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PotDonatorResponse {
    pub donator: Addr,
    pub donations: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BeneficiaryResponse {
    pub beneficiary: Addr,
    pub received_donations: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DonatorListResponse {
    pub donators: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BeneficiaryListResponse {
    pub beneficiaries: Vec<Addr>,
}
