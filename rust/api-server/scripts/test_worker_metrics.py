import unittest

from worker_metrics import parse_metrics, queued, rate


class WorkerMetricsTests(unittest.TestCase):
    def test_optional_rss_only_reads_the_resident_memory_metric(self):
        process = {}
        self.assertEqual(parse_metrics([
            'process_virtual_memory_bytes 999999999\n',
            'process_resident_memory_bytes 1048576\n',
        ], process), {})
        self.assertEqual(process, {"rss": 1048576})

    def test_filters_metrics_and_calculates_message_backlog(self):
        workers = parse_metrics([
            '# HELP worker_queue_sent_total sent messages\n',
            'unrelated_metric{worker="inventory"} 999999\n',
            'worker_queue_sent_total{worker="inventory",region="1"} 100\n',
            'worker_queue_sent_total{region="2",worker="inventory"} 20\n',
            'worker_queue_received_total{worker="inventory"} 90\n',
            'worker_queue_send_errors_total{worker="inventory"} 2\n',
            'worker_queue_receive_batch_size{worker="inventory"} 1\n',
        ])
        self.assertEqual(workers, {"inventory": {"sent": 120, "received": 90, "errors": 2, "last_batch": 1}})
        self.assertEqual(queued(workers["inventory"]), 28)

    def test_missing_counters_are_not_assumed_to_be_zero(self):
        self.assertIsNone(queued({"received": 5}))
        self.assertIsNone(queued({"sent": 5}))
        self.assertEqual(queued({"sent": 5, "received": 6}), -1)

    def test_rates_handle_counter_resets_and_missing_baselines(self):
        self.assertEqual(rate({"sent": 12}, {"sent": 2}, "sent", 5), "2.0")
        self.assertEqual(rate({"sent": 1}, {"sent": 2}, "sent", 5), "reset")
        self.assertEqual(rate({"sent": 12}, None, "sent", None), "-")

    def test_escaped_labels_and_invalid_samples(self):
        workers = parse_metrics([
            'worker_queue_sent_total{worker="a\\\"b"} 1e2 1234\n',
            'worker_queue_sent_total{worker="invalid"} NaN\n',
            'worker_queue_sent_total{worker="invalid"} nope\n',
            'worker_queue_sent_total{other="missing_worker"} 2\n',
        ])
        self.assertEqual(workers, {'a"b': {"sent": 100}})

    def test_initial_batch_metrics_are_kept_separate_from_worker_envelopes(self):
        workers = parse_metrics([
            'worker_queue_sent_total{worker="inventory_state"} 1\n',
            'worker_initial_batches_sent_total{worker="inventory_state"} 30\n',
            'worker_initial_batches_received_total{worker="inventory_state"} 20\n',
            'worker_initial_snapshots_completed_total{worker="inventory_state"} 1\n',
        ])
        self.assertEqual(workers["inventory_state"], {
            "sent": 1, "initial_sent": 30, "initial_received": 20, "initial_end": 1,
        })


if __name__ == "__main__":
    unittest.main()
