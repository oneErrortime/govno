fn main() {
    workers_common::run(workers_common::WorkerConfig {
        service_id:  "gas-shit-worker".into(),
        topic:       proto::ShitTopic::Gas,
        version:     env!("CARGO_PKG_VERSION").into(),
        interval_ms: 1200,
        description: "Производит газообразное говно с интервалом 1.2s".into(),
        priority:    proto::Priority::Normal,
        tags:        &["gas", "toxic", "cloud"],
        phrases:     &[
            "заполнило весь Kubernetes кластер",
            "просочилось через firewall",
            "в атмосфере критическая концентрация",
            "отравило продакшн окружение",
            "расширилось до размеров дата-центра",
            "технический долг испаряется в воздух",
            "CO2 от железа превысил норму",
            "облако говна дрейфует к staging",
        ],
    });
}
