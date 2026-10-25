use jig_channel_world::stories::{self, Story};
use skein_world::domain::Verdict;
#[test]
fn refused_calls_allow_later_model_work_and_empty_shutdown_across_seeds() {
    for seed in [1, 7, 19] {
        let result = stories::run(seed, Story::Call);
        assert_eq!(result.verdict, Verdict::Passed, "seed {seed}");
        assert!(result.named_call_error_observed, "seed {seed}");
        assert!(result.calls_busy > 0);
        assert_eq!(result.calls_answered, result.calls_busy + 2);
        assert_eq!((result.turns_committed, result.answers_acknowledged), (3, 1));
        assert!(result.services_empty && result.shut_down, "seed {seed}");
    }
}
