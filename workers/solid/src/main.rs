fn main() {
    workers_common::run(workers_common::WorkerConfig {
        service_id:  "solid-shit-worker".into(),
        topic:       proto::ShitTopic::Solid,
        version:     env!("CARGO_PKG_VERSION").into(),
        interval_ms: 2500,
        description: "Производит твёрдое говно с интервалом 2.5s".into(),
        priority:    proto::Priority::Normal,
        tags:        &["solid", "blocked", "stuck"],
        phrases:     &[
            "застряло в пайплайне уже 4 часа",
            "заблокировало CI/CD намертво",
            "лежит в очереди третий день",
            "не прошло code review снова",
            "упало на деплое с сегфолтом",
            "монолит не даёт себя разбить",
            "deadlock в базе данных",
            "транзакция висит 6 часов",
            "индекс не строится вторые сутки",
        ],
    });
}
