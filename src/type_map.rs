/// Maps a PostgreSQL udt_name to a ClickHouse type string.
pub fn pg_to_ch_type(pg_type: &str, is_nullable: bool, precision: Option<i32>, scale: Option<i32>) -> String {
    let base = pg_type_to_ch_base(pg_type, precision, scale);
    if is_nullable {
        format!("Nullable({})", base)
    } else {
        base
    }
}

fn pg_type_to_ch_base(pg_type: &str, precision: Option<i32>, scale: Option<i32>) -> String {
    // Array types: udt_name starts with '_'
    if let Some(element_type) = pg_type.strip_prefix('_') {
        let inner = pg_type_to_ch_base(element_type, precision, scale);
        return format!("Array({})", inner);
    }

    match pg_type {
        "int2" | "smallint"              => "Int32".to_string(),
        "int4" | "integer"               => "Int32".to_string(),
        "int8" | "bigint"                => "Int64".to_string(),
        "float4" | "real"                => "Float64".to_string(),
        "float8" | "double precision"    => "Float64".to_string(),
        "numeric" | "decimal"            => {
            let p = precision.unwrap_or(18);
            let s = scale.unwrap_or(4);
            format!("Decimal({}, {})", p, s)
        }
        "bool" | "boolean"               => "Bool".to_string(),
        "text" | "varchar" | "character varying"
        | "char" | "character" | "bpchar" | "name" => "String".to_string(),
        "uuid"                           => "UUID".to_string(),
        "timestamp" | "timestamp without time zone" => "DateTime64(6)".to_string(),
        "timestamptz" | "timestamp with time zone"  => "DateTime64(6, 'UTC')".to_string(),
        "date"                           => "Date".to_string(),
        "jsonb" | "json"                 => "String".to_string(),
        _                                => "String".to_string(),
    }
}
