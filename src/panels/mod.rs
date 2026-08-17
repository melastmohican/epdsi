//! Hardware panel specifications module.

pub mod e2154qs0f1;
pub mod e2266ks0c1;
pub mod e2290ks0f1;
pub mod e2417qs0a3;
pub mod gdem0154z90;
pub mod gdem0213b74;
pub mod gdep073e01;
pub mod gdeq0426t82;
pub mod gdey037t03;
pub mod zjy122250;

pub use e2154qs0f1::{E2154QS0F1, EPD_154_QS_0F};
pub use e2266ks0c1::{E2266KS0C1, EPD_266_KS_0C};
pub use e2290ks0f1::{E2290KS0F1, EPD_290_KS_0F};
pub use e2417qs0a3::{E2417QS0A3, EPD_417_QS_0A};
pub use gdem0154z90::GDEM0154Z90;
pub use gdem0213b74::{GxEPD2_213_B74, GDEM0213B74};
pub use gdep073e01::{GxEPD2_730c_GDEP073E01, GDEP073E01};
pub use gdeq0426t82::GDEQ0426T82;
pub use gdey037t03::{GxEPD2_370_GDEY037T03, GDEY037T03};
pub use zjy122250::{GDEY0213F51, ZJY122250_0213AJH_E5};
