fn main() {
    workers_common::run(workers_common::WorkerConfig {
        service_id:  "liquid-shit-worker".into(),
        topic:       proto::ShitTopic::Liquid,
        version:     env!("CARGO_PKG_VERSION").into(),
        interval_ms: 1800,
        description: "Производит жидкое говно с интервалом 1.8s".into(),
        priority:    proto::Priority::Normal,
        tags:        &["liquid", "flow", "leak"],
        phrases:     &[
            "растекается по всей архитектуре",
            "проникает в каждый микросервис",
            "утекает в прод прямо сейчас",
            "затопило базу данных",
            "разлилось по логам на 3 гигабайта",
            "просочилось через все абстракции",
            "memory leak достиг 4GB",
            "connection pool полностью промок",
            "SQL-запросы плывут неуправляемо",
        ],
    });
}
