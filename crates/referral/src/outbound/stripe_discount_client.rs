//! Stripe adapter for applying discounts.

use anyhow::{Context, bail};

use crate::domain::ports::DiscountClient;

/// Stripe implementation of the discount client
#[derive(Clone)]
pub struct StripeDiscountClient {
    /// inner stripe client
    stripe_client: Option<stripe::Client>,
    /// the amount a single discount is worth, in cents
    discount_amount_in_cents: i64,
}

impl StripeDiscountClient {
    /// Create a new stripe discount client
    pub fn new(stripe_client: stripe::Client, discount_amount: i64) -> Self {
        Self {
            stripe_client: Some(stripe_client),
            discount_amount_in_cents: discount_amount,
        }
    }
    /// Leave referral payments unavailable without constructing a Stripe client.
    pub fn new_optional(stripe_client: Option<stripe::Client>, discount_amount: i64) -> Self {
        Self {
            stripe_client,
            discount_amount_in_cents: discount_amount,
        }
    }
}

impl DiscountClient for StripeDiscountClient {
    type Err = anyhow::Error;

    #[tracing::instrument(skip(self), err)]
    async fn apply_discount(&self, referrer_customer_id: &str) -> Result<(), Self::Err> {
        let stripe_client = self
            .stripe_client
            .as_ref()
            .context("Stripe is unavailable: integration is not configured")?;
        let customer_id: stripe::CustomerId = referrer_customer_id
            .parse()
            .context("invalid stripe customer id format")?;

        let customer = stripe::Customer::retrieve(stripe_client, &customer_id, &[])
            .await
            .inspect_err(|e| tracing::error!(error=?e, "failed to retrieve stripe customer"))
            .context("stripe customer not found or could not be retrieved")?;

        if customer.deleted {
            bail!("stripe customer '{referrer_customer_id}' has been deleted");
        }

        // Negative amount = credit applied to the customer's balance
        let params = stripe::CreateCustomerBalanceTransaction {
            amount: -self.discount_amount_in_cents,
            currency: stripe::Currency::USD,
            description: Some("Referral credit"),
            metadata: None,
        };

        stripe::Customer::create_balance_transaction(stripe_client, &customer_id, params)
            .await
            .inspect_err(
                |e| tracing::error!(error=?e, "failed to create balance transaction for customer"),
            )
            .context("failed to apply referral credit to stripe customer balance")?;

        Ok(())
    }
}
