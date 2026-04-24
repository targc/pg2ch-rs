-- ============================================================
-- Schema
-- ============================================================

CREATE TABLE users (
    id         SERIAL PRIMARY KEY,
    name       TEXT NOT NULL,
    email      TEXT NOT NULL UNIQUE,
    age        INT,
    is_active  BOOLEAN NOT NULL DEFAULT true,
    score      NUMERIC(10, 2),
    metadata   JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP
);

CREATE TABLE orders (
    id          SERIAL PRIMARY KEY,
    user_id     INT NOT NULL REFERENCES users(id),
    status      TEXT NOT NULL DEFAULT 'pending',
    total       NUMERIC(12, 2) NOT NULL,
    notes       TEXT,
    created_at  TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE transactions (
    id           SERIAL PRIMARY KEY,
    order_id     INT NOT NULL REFERENCES orders(id),
    amount       NUMERIC(12, 2) NOT NULL,
    currency     TEXT NOT NULL DEFAULT 'USD',
    reference_id UUID NOT NULL DEFAULT gen_random_uuid(),
    created_at   TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ============================================================
-- Data — users
-- ============================================================

INSERT INTO users (name, email, age, is_active, score, metadata, created_at, updated_at) VALUES
    ('Alice',   'alice@example.com',   28, true,  95.50, '{"plan": "pro",  "country": "US"}', '2024-01-01 08:00:00', '2024-06-10 12:00:00'),
    ('Bob',     'bob@example.com',     34, true,  72.00, '{"plan": "free", "country": "UK"}', '2024-01-05 09:30:00', '2024-07-01 09:00:00'),
    ('Carol',   'carol@example.com',   25, false, 40.25, '{"plan": "free", "country": "AU"}', '2024-02-14 10:00:00', '2024-07-15 14:00:00'),
    ('Dave',    'dave@example.com',    41, true,  88.00, '{"plan": "pro",  "country": "CA"}', '2024-03-01 11:00:00', '2024-08-01 08:30:00'),
    ('Eve',     'eve@example.com',     30, true,  NULL,  NULL,                                '2024-03-20 12:00:00', '2024-08-05 10:00:00'),
    ('Frank',   'frank@example.com',   55, false, 61.75, '{"plan": "pro",  "country": "DE"}', '2024-04-01 07:00:00', '2024-08-10 16:00:00'),
    ('Grace',   'grace@example.com',   22, true,  99.99, '{"plan": "pro",  "country": "JP"}', '2024-04-15 08:30:00', '2024-09-01 11:00:00'),
    ('Henry',   'henry@example.com',   38, true,  50.00, '{"plan": "free", "country": "US"}', '2024-05-01 09:00:00', '2024-09-10 13:00:00'),
    ('Iris',    'iris@example.com',    27, true,  83.40, '{"plan": "pro",  "country": "FR"}', '2024-05-20 10:30:00', '2024-09-20 09:00:00'),
    ('Jack',    'jack@example.com',    45, false, 30.00, '{"plan": "free", "country": "BR"}', '2024-06-01 11:00:00', '2024-10-01 08:00:00');

-- ============================================================
-- Data — orders
-- ============================================================

INSERT INTO orders (user_id, status, total, notes, created_at, updated_at) VALUES
    (1, 'completed', 120.00, 'First order',        '2024-02-01 10:00:00', '2024-02-03 12:00:00'),
    (1, 'completed', 350.50, NULL,                 '2024-04-10 11:00:00', '2024-04-12 09:00:00'),
    (2, 'completed',  89.99, 'Gift wrap requested','2024-03-05 09:00:00', '2024-03-07 14:00:00'),
    (2, 'pending',   210.00, NULL,                 '2024-07-20 10:30:00', '2024-07-20 10:30:00'),
    (3, 'cancelled',  45.00, 'Customer changed mind','2024-05-01 08:00:00','2024-05-02 10:00:00'),
    (4, 'completed', 999.00, 'Bulk order',         '2024-06-15 13:00:00', '2024-06-18 16:00:00'),
    (5, 'completed',  55.25, NULL,                 '2024-07-01 09:00:00', '2024-07-02 11:00:00'),
    (6, 'pending',   175.80, NULL,                 '2024-08-05 10:00:00', '2024-08-05 10:00:00'),
    (7, 'completed', 430.00, 'Priority shipping',  '2024-08-20 14:00:00', '2024-08-22 09:00:00'),
    (8, 'completed',  22.50, NULL,                 '2024-09-01 08:30:00', '2024-09-01 15:00:00'),
    (9, 'completed', 310.75, NULL,                 '2024-09-15 12:00:00', '2024-09-17 10:00:00'),
    (10,'pending',    67.00, NULL,                 '2024-10-01 09:00:00', '2024-10-01 09:00:00');

-- ============================================================
-- Data — transactions
-- ============================================================

INSERT INTO transactions (order_id, amount, currency, created_at) VALUES
    (1,  120.00, 'USD', '2024-02-03 12:00:00'),
    (2,  350.50, 'USD', '2024-04-12 09:00:00'),
    (3,   89.99, 'GBP', '2024-03-07 14:00:00'),
    (6,  999.00, 'CAD', '2024-06-18 16:00:00'),
    (7,   55.25, 'USD', '2024-07-02 11:00:00'),
    (9,  430.00, 'JPY', '2024-08-22 09:00:00'),
    (10,  22.50, 'USD', '2024-09-01 15:00:00'),
    (11, 310.75, 'EUR', '2024-09-17 10:00:00');
