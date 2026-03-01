//! Double-entry accounting ledger backed by rustledger-core.
//!
//! Every cash mutation in the game is recorded as a balanced beancount
//! [`Transaction`] with two [`Posting`]s. The ledger is the authoritative
//! source of truth for all financial data; `GameState.cash` (f32) is a
//! real-time cache verified against the ledger at each day boundary.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rustledger_core::{Amount, Posting, Transaction};
use thiserror::Error;

// ============ Chart of Accounts ============

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Account {
    // Assets
    Cash,
    Equipment,
    // Equity
    Opening,
    // Income
    Charging,
    Ads,
    SolarExport,
    CarbonCredits,
    // Expenses — Energy
    Energy,
    DemandCharge,
    // Expenses — Operations
    Opex,
    CableTheft,
    Warranty,
    WarrantyRecovery,
    Refunds,
    Penalties,
    // Expenses — Fixed
    Rent,
    Upgrades,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExpenseCategory {
    Energy,
    Operations,
    Fixed,
}

impl Account {
    pub const ALL_INCOME: &[Account] = &[
        Account::Charging,
        Account::Ads,
        Account::SolarExport,
        Account::CarbonCredits,
    ];

    pub const ALL_EXPENSES: &[Account] = &[
        Account::Energy,
        Account::DemandCharge,
        Account::Opex,
        Account::CableTheft,
        Account::Warranty,
        Account::WarrantyRecovery,
        Account::Refunds,
        Account::Penalties,
        Account::Rent,
        Account::Upgrades,
    ];

    pub fn beancount_name(&self) -> &'static str {
        match self {
            Self::Cash => "Assets:Cash",
            Self::Equipment => "Assets:Equipment",
            Self::Opening => "Equity:Opening",
            Self::Charging => "Income:Charging",
            Self::Ads => "Income:Ads",
            Self::SolarExport => "Income:SolarExport",
            Self::CarbonCredits => "Income:CarbonCredits",
            Self::Energy => "Expenses:Energy",
            Self::DemandCharge => "Expenses:DemandCharge",
            Self::Opex => "Expenses:Opex",
            Self::CableTheft => "Expenses:CableTheft",
            Self::Warranty => "Expenses:Warranty",
            Self::WarrantyRecovery => "Expenses:WarrantyRecovery",
            Self::Refunds => "Expenses:Refunds",
            Self::Penalties => "Expenses:Penalties",
            Self::Rent => "Expenses:Rent",
            Self::Upgrades => "Expenses:Upgrades",
        }
    }

    pub fn from_beancount_name(name: &str) -> Option<Self> {
        match name {
            "Assets:Cash" => Some(Self::Cash),
            "Assets:Equipment" => Some(Self::Equipment),
            "Equity:Opening" => Some(Self::Opening),
            "Income:Charging" => Some(Self::Charging),
            "Income:Ads" => Some(Self::Ads),
            "Income:SolarExport" => Some(Self::SolarExport),
            "Income:CarbonCredits" => Some(Self::CarbonCredits),
            "Expenses:Energy" => Some(Self::Energy),
            "Expenses:DemandCharge" => Some(Self::DemandCharge),
            "Expenses:Opex" => Some(Self::Opex),
            "Expenses:CableTheft" => Some(Self::CableTheft),
            "Expenses:Warranty" => Some(Self::Warranty),
            "Expenses:WarrantyRecovery" => Some(Self::WarrantyRecovery),
            "Expenses:Refunds" => Some(Self::Refunds),
            "Expenses:Penalties" => Some(Self::Penalties),
            "Expenses:Rent" => Some(Self::Rent),
            "Expenses:Upgrades" => Some(Self::Upgrades),
            _ => None,
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Cash => "Cash",
            Self::Equipment => "Equipment",
            Self::Opening => "Opening Balance",
            Self::Charging => "Charging",
            Self::Ads => "Ads",
            Self::SolarExport => "Solar Export",
            Self::CarbonCredits => "Carbon Credits",
            Self::Energy => "Energy",
            Self::DemandCharge => "Demand Charge",
            Self::Opex => "Opex",
            Self::CableTheft => "Cable Theft",
            Self::Warranty => "Warranty",
            Self::WarrantyRecovery => "Warranty Recovery",
            Self::Refunds => "Refunds",
            Self::Penalties => "Penalties",
            Self::Rent => "Rent",
            Self::Upgrades => "Upgrades",
        }
    }

    pub fn is_income(&self) -> bool {
        matches!(
            self,
            Self::Charging | Self::Ads | Self::SolarExport | Self::CarbonCredits
        )
    }

    pub fn is_expense(&self) -> bool {
        self.expense_category().is_some() && !self.is_contra()
    }

    /// Contra-expense accounts offset expenses within their category
    /// (e.g. WarrantyRecovery reduces Operations total).
    pub fn is_contra(&self) -> bool {
        matches!(self, Self::WarrantyRecovery)
    }

    pub fn expense_category(&self) -> Option<ExpenseCategory> {
        match self {
            Self::Energy | Self::DemandCharge => Some(ExpenseCategory::Energy),
            Self::Opex
            | Self::CableTheft
            | Self::Warranty
            | Self::WarrantyRecovery
            | Self::Refunds
            | Self::Penalties => Some(ExpenseCategory::Operations),
            Self::Rent | Self::Upgrades => Some(ExpenseCategory::Fixed),
            _ => None,
        }
    }
}

const USD: &str = "USD";

// ============ Errors ============

#[derive(Error, Debug)]
pub enum LedgerError {
    #[error(
        "cash balance mismatch: ledger has {ledger_balance}, game state has {game_state_balance} (delta {delta})"
    )]
    CashMismatch {
        ledger_balance: Decimal,
        game_state_balance: Decimal,
        delta: Decimal,
    },
}

// ============ Daily financial snapshot ============

/// All financial fields for a single day, derived from the ledger.
#[derive(Debug, Clone, Default)]
pub struct DailyFinancials {
    pub charging_revenue: f32,
    pub ad_revenue: f32,
    pub solar_export_revenue: f32,
    pub carbon_credits: f32,
    pub energy_cost: f32,
    pub demand_charge: f32,
    pub opex: f32,
    pub cable_theft_cost: f32,
    pub warranty_cost: f32,
    pub warranty_recovery: f32,
    pub refunds: f32,
    pub penalties: f32,
    pub rent: f32,
    pub upgrades: f32,
    pub capex: f32,
    pub capex_refund: f32,
}

impl DailyFinancials {
    /// Look up the value for a specific account.
    pub fn get(&self, account: Account) -> f32 {
        match account {
            Account::Charging => self.charging_revenue,
            Account::Ads => self.ad_revenue,
            Account::SolarExport => self.solar_export_revenue,
            Account::CarbonCredits => self.carbon_credits,
            Account::Energy => self.energy_cost,
            Account::DemandCharge => self.demand_charge,
            Account::Opex => self.opex,
            Account::CableTheft => self.cable_theft_cost,
            Account::Warranty => self.warranty_cost,
            Account::WarrantyRecovery => self.warranty_recovery,
            Account::Refunds => self.refunds,
            Account::Penalties => self.penalties,
            Account::Rent => self.rent,
            Account::Upgrades => self.upgrades,
            Account::Cash | Account::Equipment | Account::Opening => 0.0,
        }
    }

    /// Compute the net total for an expense category, automatically
    /// subtracting contra-expense accounts (e.g. WarrantyRecovery).
    pub fn category_total(&self, category: ExpenseCategory) -> f32 {
        let mut total = 0.0;
        for &acct in Account::ALL_EXPENSES {
            if acct.expense_category() == Some(category) {
                if acct.is_contra() {
                    total -= self.get(acct);
                } else {
                    total += self.get(acct);
                }
            }
        }
        total
    }
}

// ============ Ledger ============

/// Double-entry accounting ledger.
///
/// Records every financial transaction as a balanced beancount
/// `Transaction` and maintains cached running totals for
/// efficient per-frame UI reads.
#[derive(Debug, Clone)]
pub struct Ledger {
    pub journal: Vec<Transaction>,
    pub current_date: NaiveDate,

    // Cached running totals (updated on every record_* call)
    cache_cash: Decimal,
    cache_gross_revenue: Decimal,
    cache_total_expenses: Decimal,
    cache_total_opex: Decimal,
    cache_equipment: Decimal,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            journal: Vec::new(),
            current_date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
            cache_cash: Decimal::ZERO,
            cache_gross_revenue: Decimal::ZERO,
            cache_total_expenses: Decimal::ZERO,
            cache_total_opex: Decimal::ZERO,
            cache_equipment: Decimal::ZERO,
        }
    }
}

impl Ledger {
    /// Create a new ledger with an opening balance transaction.
    pub fn with_opening_balance(starting_cash: f32) -> Self {
        let mut ledger = Self::default();
        let amount = to_decimal(starting_cash);
        let txn = Transaction::new(ledger.current_date, "Opening balance")
            .with_posting(Posting::new(
                Account::Cash.beancount_name(),
                Amount::new(amount, USD),
            ))
            .with_posting(Posting::new(
                Account::Opening.beancount_name(),
                Amount::new(-amount, USD),
            ));
        ledger.cache_cash += amount;
        ledger.journal.push(txn);
        ledger
    }

    // ============ Recording helpers ============

    /// Record revenue: debit Cash, credit an Income account.
    pub fn record_revenue(&mut self, amount: f32, account: Account, narration: &str) {
        if amount <= 0.0 {
            return;
        }
        let dec = to_decimal(amount);
        if dec.is_zero() {
            return;
        }
        let txn = Transaction::new(self.current_date, narration)
            .with_posting(Posting::new(
                Account::Cash.beancount_name(),
                Amount::new(dec, USD),
            ))
            .with_posting(Posting::new(
                account.beancount_name(),
                Amount::new(-dec, USD),
            ));
        self.cache_cash += dec;
        self.cache_gross_revenue += dec;
        self.journal.push(txn);
    }

    /// Record expense: debit an Expense account, credit Cash.
    pub fn record_expense(&mut self, amount: f32, account: Account, narration: &str) {
        if amount <= 0.0 {
            return;
        }
        let dec = to_decimal(amount);
        if dec.is_zero() {
            return;
        }
        let txn = Transaction::new(self.current_date, narration)
            .with_posting(Posting::new(
                account.beancount_name(),
                Amount::new(dec, USD),
            ))
            .with_posting(Posting::new(
                Account::Cash.beancount_name(),
                Amount::new(-dec, USD),
            ));
        self.cache_cash -= dec;
        self.cache_total_expenses += dec;
        if account.expense_category() == Some(ExpenseCategory::Operations) {
            self.cache_total_opex += dec;
        }
        self.journal.push(txn);
    }

    /// Record a contra-expense: debit Cash, credit a contra-expense account.
    /// Reduces net expenses and the account's category total.
    pub fn record_contra_expense(&mut self, amount: f32, account: Account, narration: &str) {
        if amount <= 0.0 {
            return;
        }
        let dec = to_decimal(amount);
        if dec.is_zero() {
            return;
        }
        let txn = Transaction::new(self.current_date, narration)
            .with_posting(Posting::new(
                Account::Cash.beancount_name(),
                Amount::new(dec, USD),
            ))
            .with_posting(Posting::new(
                account.beancount_name(),
                Amount::new(-dec, USD),
            ));
        self.cache_cash += dec;
        self.cache_total_expenses -= dec;
        if account.expense_category() == Some(ExpenseCategory::Operations) {
            self.cache_total_opex -= dec;
        }
        self.journal.push(txn);
    }

    /// Record capital expenditure (building equipment): debit Equipment, credit Cash.
    pub fn record_capex(&mut self, amount: f32, narration: &str) {
        if amount <= 0.0 {
            return;
        }
        let dec = to_decimal(amount);
        let txn = Transaction::new(self.current_date, narration)
            .with_posting(Posting::new(
                Account::Equipment.beancount_name(),
                Amount::new(dec, USD),
            ))
            .with_posting(Posting::new(
                Account::Cash.beancount_name(),
                Amount::new(-dec, USD),
            ));
        self.cache_cash -= dec;
        self.cache_equipment += dec;
        self.journal.push(txn);
    }

    /// Record capital expenditure refund (selling equipment): debit Cash, credit Equipment.
    pub fn record_capex_refund(&mut self, amount: f32, narration: &str) {
        if amount <= 0.0 {
            return;
        }
        let dec = to_decimal(amount);
        let txn = Transaction::new(self.current_date, narration)
            .with_posting(Posting::new(
                Account::Cash.beancount_name(),
                Amount::new(dec, USD),
            ))
            .with_posting(Posting::new(
                Account::Equipment.beancount_name(),
                Amount::new(-dec, USD),
            ));
        self.cache_cash += dec;
        self.cache_equipment -= dec;
        self.journal.push(txn);
    }

    // ============ Cached query methods ============

    /// Current cash balance (Decimal precision).
    pub fn cash_balance(&self) -> Decimal {
        self.cache_cash
    }

    /// Cash balance as f32 for game-state comparison.
    pub fn cash_balance_f32(&self) -> f32 {
        decimal_to_f32(self.cache_cash)
    }

    /// Cumulative gross revenue (all Income credits).
    pub fn gross_revenue(&self) -> Decimal {
        self.cache_gross_revenue
    }

    pub fn gross_revenue_f32(&self) -> f32 {
        decimal_to_f32(self.cache_gross_revenue)
    }

    /// Cumulative net revenue (gross revenue minus net expenses).
    pub fn net_revenue(&self) -> Decimal {
        self.cache_gross_revenue - self.cache_total_expenses
    }

    pub fn net_revenue_f32(&self) -> f32 {
        decimal_to_f32(self.net_revenue())
    }

    /// Cumulative net Operations category total (expenses minus contra-expenses).
    pub fn total_opex(&self) -> Decimal {
        self.cache_total_opex
    }

    pub fn total_opex_f32(&self) -> f32 {
        decimal_to_f32(self.cache_total_opex)
    }

    /// Cumulative total expenses (all Expense accounts, net of contra-expenses).
    pub fn total_expenses(&self) -> Decimal {
        self.cache_total_expenses
    }

    /// Cumulative equipment balance.
    pub fn equipment_balance(&self) -> Decimal {
        self.cache_equipment
    }

    // ============ Day-boundary queries ============

    /// Compute the cumulative balance for a specific account by scanning the journal.
    pub fn account_balance(&self, account: Account) -> Decimal {
        let name = account.beancount_name();
        let mut total = Decimal::ZERO;
        for txn in &self.journal {
            for posting in &txn.postings {
                if posting.account.as_ref() == name
                    && let Some(amt) = posting.amount()
                {
                    total += amt.number;
                }
            }
        }
        total
    }

    /// Compute all financial fields for a single day from the journal.
    pub fn daily_totals(&self, date: NaiveDate) -> DailyFinancials {
        let mut f = DailyFinancials::default();
        for txn in &self.journal {
            if txn.date != date {
                continue;
            }
            for posting in &txn.postings {
                let Some(acct) = Account::from_beancount_name(posting.account.as_ref()) else {
                    continue;
                };
                let Some(amt) = posting.amount() else {
                    continue;
                };
                let val = decimal_to_f32(amt.number.abs());
                match acct {
                    Account::Charging => f.charging_revenue += val,
                    Account::Ads => f.ad_revenue += val,
                    Account::SolarExport => f.solar_export_revenue += val,
                    Account::CarbonCredits => f.carbon_credits += val,
                    Account::Energy => f.energy_cost += val,
                    Account::DemandCharge => f.demand_charge += val,
                    Account::Opex => f.opex += val,
                    Account::CableTheft => f.cable_theft_cost += val,
                    Account::Warranty => f.warranty_cost += val,
                    Account::WarrantyRecovery => f.warranty_recovery += val,
                    Account::Refunds => f.refunds += val,
                    Account::Penalties => f.penalties += val,
                    Account::Rent => f.rent += val,
                    Account::Upgrades => f.upgrades += val,
                    Account::Equipment => {
                        if amt.number.is_sign_positive() {
                            f.capex += val;
                        } else {
                            f.capex_refund += val;
                        }
                    }
                    Account::Cash | Account::Opening => {}
                }
            }
        }
        f
    }

    // ============ Verification ============

    /// Verify the ledger cash balance matches the game state cash (within f32 tolerance).
    pub fn verify_cash(&self, game_state_cash: f32) -> Result<(), LedgerError> {
        let gs_dec = to_decimal(game_state_cash);
        let delta = (self.cache_cash - gs_dec).abs();
        // Cash is synced from the ledger at each checkpoint; only tiny
        // Decimal→f32 conversion jitter should remain.
        if delta > Decimal::new(1, 1) {
            return Err(LedgerError::CashMismatch {
                ledger_balance: self.cache_cash,
                game_state_balance: gs_dec,
                delta,
            });
        }
        Ok(())
    }

    /// Check if the journal is balanced (sum of all postings across all transactions == 0).
    pub fn is_balanced(&self) -> bool {
        let mut total = Decimal::ZERO;
        for txn in &self.journal {
            for posting in &txn.postings {
                if let Some(amt) = posting.amount() {
                    total += amt.number;
                }
            }
        }
        total.is_zero()
    }

    /// Number of transactions in the journal.
    pub fn transaction_count(&self) -> usize {
        self.journal.len()
    }

    /// Reset the ledger (for game restart).
    pub fn reset(&mut self, starting_cash: f32) {
        *self = Self::with_opening_balance(starting_cash);
    }
}

// ============ Helpers ============

fn to_decimal(value: f32) -> Decimal {
    Decimal::from_f32_retain(value)
        .unwrap_or(Decimal::ZERO)
        .round_dp(2)
}

fn decimal_to_f32(value: Decimal) -> f32 {
    use rust_decimal::prelude::ToPrimitive;
    value.to_f32().unwrap_or(0.0)
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opening_balance() {
        let ledger = Ledger::with_opening_balance(1_000_000.0);
        assert_eq!(ledger.cash_balance(), to_decimal(1_000_000.0));
        assert_eq!(ledger.transaction_count(), 1);
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_revenue_recording() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        ledger.record_revenue(100.0, Account::Charging, "Charging session");

        assert_eq!(ledger.cash_balance(), to_decimal(1100.0));
        assert_eq!(ledger.gross_revenue(), to_decimal(100.0));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_expense_recording() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        ledger.record_expense(50.0, Account::Energy, "Grid electricity");

        assert_eq!(ledger.cash_balance(), to_decimal(950.0));
        assert_eq!(ledger.total_expenses(), to_decimal(50.0));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_capex_recording() {
        let mut ledger = Ledger::with_opening_balance(100_000.0);
        ledger.record_capex(40_000.0, "DCFC charger purchase");

        assert_eq!(ledger.cash_balance(), to_decimal(60_000.0));
        assert_eq!(ledger.equipment_balance(), to_decimal(40_000.0));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_capex_refund() {
        let mut ledger = Ledger::with_opening_balance(100_000.0);
        ledger.record_capex(40_000.0, "Buy charger");
        ledger.record_capex_refund(20_000.0, "Sell charger (50%)");

        assert_eq!(ledger.cash_balance(), to_decimal(80_000.0));
        assert_eq!(ledger.equipment_balance(), to_decimal(20_000.0));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_net_revenue() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        ledger.record_revenue(500.0, Account::Charging, "Session");
        ledger.record_expense(200.0, Account::Energy, "Energy");
        ledger.record_expense(50.0, Account::Opex, "Maintenance");

        assert_eq!(ledger.gross_revenue(), to_decimal(500.0));
        assert_eq!(ledger.net_revenue(), to_decimal(250.0));
        assert_eq!(ledger.total_opex(), to_decimal(50.0));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_daily_totals() {
        let mut ledger = Ledger::with_opening_balance(10_000.0);
        let day1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();

        ledger.current_date = day1;
        ledger.record_revenue(300.0, Account::Charging, "Day 1 charging");
        ledger.record_expense(100.0, Account::Energy, "Day 1 energy");

        ledger.current_date = day2;
        ledger.record_revenue(500.0, Account::Charging, "Day 2 charging");
        ledger.record_expense(200.0, Account::Energy, "Day 2 energy");

        let d1 = ledger.daily_totals(day1);
        assert!((d1.charging_revenue - 300.0).abs() < 0.01);
        assert!((d1.energy_cost - 100.0).abs() < 0.01);

        let d2 = ledger.daily_totals(day2);
        assert!((d2.charging_revenue - 500.0).abs() < 0.01);
        assert!((d2.energy_cost - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_verify_cash_ok() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        ledger.record_revenue(100.0, Account::Charging, "Session");
        ledger.record_expense(50.0, Account::Energy, "Energy");
        assert!(ledger.verify_cash(1050.0).is_ok());
    }

    #[test]
    fn test_verify_cash_mismatch() {
        let ledger = Ledger::with_opening_balance(1000.0);
        assert!(ledger.verify_cash(500.0).is_err());
    }

    #[test]
    fn test_zero_amount_ignored() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        let count_before = ledger.transaction_count();
        ledger.record_revenue(0.0, Account::Charging, "Zero");
        ledger.record_expense(-5.0, Account::Energy, "Negative");
        assert_eq!(ledger.transaction_count(), count_before);
    }

    #[test]
    fn test_reset() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        ledger.record_revenue(500.0, Account::Charging, "Session");
        ledger.reset(2000.0);

        assert_eq!(ledger.cash_balance(), to_decimal(2000.0));
        assert_eq!(ledger.gross_revenue(), Decimal::ZERO);
        assert_eq!(ledger.transaction_count(), 1);
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_full_day_simulation() {
        let mut ledger = Ledger::with_opening_balance(1_000_000.0);

        ledger.record_capex(80_000.0, "DCFC150 charger");
        ledger.record_capex(3_000.0, "L2 charger");

        ledger.record_revenue(1164.63, Account::Charging, "Charging sessions");
        ledger.record_revenue(48.0, Account::Ads, "Ad revenue");
        ledger.record_revenue(12.50, Account::SolarExport, "Solar export");
        ledger.record_revenue(8.75, Account::CarbonCredits, "Carbon credits");

        ledger.record_expense(776.21, Account::Energy, "Grid electricity");
        ledger.record_expense(216.66, Account::DemandCharge, "Demand charges");
        ledger.record_expense(120.0, Account::Opex, "Maintenance");
        ledger.record_expense(350.0, Account::CableTheft, "Cable replacement");
        ledger.record_expense(15.0, Account::Warranty, "Warranty premium");
        ledger.record_expense(25.0, Account::Refunds, "Customer refund");
        ledger.record_expense(50.0, Account::Penalties, "Ticket penalty");

        assert!(ledger.is_balanced());

        let expected_cash = 1_000_000.0 - 80_000.0 - 3_000.0 + 1164.63 + 48.0 + 12.50 + 8.75
            - 776.21
            - 216.66
            - 120.0
            - 350.0
            - 15.0
            - 25.0
            - 50.0;
        assert!(ledger.verify_cash(expected_cash).is_ok());
    }

    #[test]
    fn test_account_balance() {
        let mut ledger = Ledger::with_opening_balance(1000.0);
        ledger.record_revenue(100.0, Account::Charging, "Session 1");
        ledger.record_revenue(200.0, Account::Charging, "Session 2");

        // Income accounts have credit-normal (negative) balances in beancount
        let charging_balance = ledger.account_balance(Account::Charging);
        assert_eq!(charging_balance, to_decimal(-300.0));

        let cash_balance = ledger.account_balance(Account::Cash);
        assert_eq!(cash_balance, to_decimal(1300.0));
    }

    #[test]
    fn test_contra_expense_reduces_opex() {
        let mut ledger = Ledger::with_opening_balance(10_000.0);
        ledger.record_expense(1000.0, Account::Opex, "Full repair cost");
        ledger.record_contra_expense(750.0, Account::WarrantyRecovery, "Warranty coverage");

        assert_eq!(ledger.total_opex(), to_decimal(250.0));
        assert_eq!(ledger.total_expenses(), to_decimal(250.0));
        assert_eq!(ledger.cash_balance(), to_decimal(9750.0));
        assert!(ledger.is_balanced());
    }

    #[test]
    fn test_category_totals() {
        let mut ledger = Ledger::with_opening_balance(100_000.0);
        ledger.record_expense(500.0, Account::Energy, "Grid electricity");
        ledger.record_expense(100.0, Account::DemandCharge, "Demand charge");
        ledger.record_expense(800.0, Account::Opex, "Repairs");
        ledger.record_expense(50.0, Account::Warranty, "Premium");
        ledger.record_contra_expense(600.0, Account::WarrantyRecovery, "Claim");
        ledger.record_expense(200.0, Account::Rent, "Site rent");

        let day = ledger.current_date;
        let f = ledger.daily_totals(day);

        let energy_total = f.category_total(ExpenseCategory::Energy);
        assert!((energy_total - 600.0).abs() < 0.01);

        let ops_total = f.category_total(ExpenseCategory::Operations);
        assert!((ops_total - 250.0).abs() < 0.01); // 800 + 50 - 600

        let fixed_total = f.category_total(ExpenseCategory::Fixed);
        assert!((fixed_total - 200.0).abs() < 0.01);
    }
}
