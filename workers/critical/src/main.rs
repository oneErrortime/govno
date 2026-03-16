fn main() {
    workers_common::run(workers_common::WorkerConfig {
        service_id:  "critical-shit-worker".into(),
        topic:       proto::ShitTopic::Critical,
        version:     env!("CARGO_PKG_VERSION").into(),
        interval_ms: 15000,
        description: "КРИТИЧЕСКИЕ инциденты. Редко, но метко.".into(),
        priority:    proto::Priority::Critical,
        tags:        &["critical", "incident", "pager"],
        phrases:     &[
            "БАЗА ДАННЫХ УТОНУЛА В ГОВНЕ",
            "PRODUCTION DOWN: сегфолт в оркестраторе",
            "MEMORY: 128GB говна и растёт",
            "DISK FULL: /var/log/govno заполнен",
            "KERNEL PANIC: говно переполнило стек ядра",
            "FIRE: серверная горит, говно кипит",
            "CEO звонит — прод лежит третий час",
        ],
    });
}
