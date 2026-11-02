SET lock_timeout = '5s';
ALTER TABLE macro_user ALTER COLUMN stripe_customer_id DROP NOT NULL;
