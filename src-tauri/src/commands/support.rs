use super::*;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;

mod agent_skills;
mod agent_tools;
mod ai_provider;
mod analytics;
mod asset_upload;
mod backup;
mod business_queries;
mod category_attributes;
mod category_catalog;
mod delivery_helpers;
mod evidence_export;
mod job_status;
#[cfg(test)]
mod pricing_strategy_tests;
mod product_status;
mod publish_loaders;
mod publish_payload;
mod settings;
mod shipment_helpers;
mod supplier_agent;
mod sync_persistence;
mod validation;
mod wechat_auth;

pub(in crate::commands) use agent_skills::*;
pub(crate) use agent_tools::{
    get_agent_active_categories, get_agent_after_sale_addresses, get_agent_aftersale,
    get_agent_category_detail, get_agent_category_tree, get_agent_delivery_companies,
    get_agent_freight_templates, get_agent_inventory_risk, get_agent_order,
    get_agent_product_detail, get_agent_product_sales, get_agent_profit_summary,
    get_agent_purchase_task, get_agent_reject_reasons, get_agent_shop_info, get_agent_shop_product,
    list_agent_aftersales, list_agent_collections, list_agent_orders, search_agent_categories,
    search_agent_docs,
};
pub(in crate::commands) use ai_provider::*;
pub(in crate::commands) use analytics::*;
pub(in crate::commands) use asset_upload::*;
pub(in crate::commands) use backup::*;
pub(in crate::commands) use business_queries::*;
pub(in crate::commands) use category_attributes::*;
pub(in crate::commands) use category_catalog::*;
pub(in crate::commands) use delivery_helpers::*;
pub(in crate::commands) use evidence_export::*;
pub(in crate::commands) use job_status::*;
pub(in crate::commands) use product_status::*;
pub(in crate::commands) use publish_loaders::*;
pub(in crate::commands) use publish_payload::*;
pub(in crate::commands) use settings::*;
pub(in crate::commands) use shipment_helpers::*;
pub(in crate::commands) use supplier_agent::*;
pub(in crate::commands) use sync_persistence::*;
pub(in crate::commands) use validation::*;
pub(in crate::commands) use wechat_auth::*;
