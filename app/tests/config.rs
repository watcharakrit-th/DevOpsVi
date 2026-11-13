use std::time::Duration;

use miniarcade::config::Config;

#[test]
fn defaults_match_the_spec() {
    let config = Config::from_lookup(|_| None).unwrap();
    assert_eq!(config.redis_url, "redis://127.0.0.1:6379");
    assert_eq!(config.app_addr, "0.0.0.0:8080".parse().unwrap());
    assert_eq!(config.ops_addr, "0.0.0.0:9090".parse().unwrap());
    assert_eq!(config.drain, Duration::from_secs(5));
}

#[test]
fn settings_come_from_the_environment() {
    let config = Config::from_lookup(|name| {
        let value = match name {
            "REDIS_URL" => "redis://redis:6379",
            "APP_ADDR" => "127.0.0.1:18080",
            "OPS_ADDR" => "127.0.0.1:19090",
            "DRAIN_SECONDS" => "0",
            _ => return None,
        };
        Some(value.to_owned())
    })
    .unwrap();
    assert_eq!(config.redis_url, "redis://redis:6379");
    assert_eq!(config.app_addr, "127.0.0.1:18080".parse().unwrap());
    assert_eq!(config.ops_addr, "127.0.0.1:19090".parse().unwrap());
    assert_eq!(config.drain, Duration::ZERO);
}

#[test]
fn bad_values_are_reported_by_name() {
    for name in ["APP_ADDR", "OPS_ADDR", "DRAIN_SECONDS"] {
        let error =
            Config::from_lookup(|key| (key == name).then(|| "nope".to_owned())).unwrap_err();
        assert!(error.contains(name), "{error}");
    }
}
