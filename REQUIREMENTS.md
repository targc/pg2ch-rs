# pg2ch

is container-based deployment for syning postgresql to clickhouse by using micro-batch concept with cursor.

for first run it will read source table schemas and recheck if destination db already have it, if not it will auto create table on clickhouse (mapping column types is important).

it will interval (not cron, ticker). means it will wait task to sync done first then wait for next interval.

they will written in rust.

for example main configurations (yaml)

interval_ms: 5000
query_batch_size: 100
upsert_batch_size: 100
source:
    connection_url: xxx
destination:
    connection_url: xxx
    table: example_users
schemas:
    - table: example_users
      cursors:
        - updated_at
        - id
    - table: example_orders
      cursors:
        - updated_at
        - id
    - table: example_transactions
      cursors:
        - created_at
        - id

