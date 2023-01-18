BEGIN;
    UPDATE subscriptions
        SET STATUS = 'confirmed'
        WHERE STATUS IS NULL;
    ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
COMMIT; 
