const LIMITS: skein_ui::Limits = skein_ui::Limits {
    nodes: 12,
    depth: 4,
    text_per_node: 32,
    text_bytes: 128,
    drawing_bytes: 8,
    styles_per_node: 2,
    patches: 72,
    bindings: 4,
    slots: 4,
    markdown_depth: 2,
    scene_marks: 0,
    scene_bytes: 0,
};

#[test]
fn jig_worst_case_adds_the_page_scratch_to_skein_uis() {
    let ui = skein_ui::worst_case(&LIMITS).expect("small UI limits fit");
    let pages = crate::pages::scratch(&LIMITS).expect("small page limits fit");
    assert_eq!(crate::limits::jig_worst_case(&LIMITS), Some(ui + pages));
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    use crate::{Limits, valid};
    use skein_lib::Duration;
    let base = super::domain::spine::LIMITS;
    assert!(valid(&base));
    let cases = [
        Limits { watches: 0, ..base },
        Limits { notice_life: Duration::ZERO, ..base },
        Limits { linger: Duration::ZERO, ..base },
        Limits { save_delay: Duration::ZERO, ..base },
        Limits { sources: 0, ..base },
        Limits { conversation_words: base.intents - 1, ..base },
        Limits { reads: u32::MAX, ..base },
        Limits { watches: u32::MAX, ..base },
        Limits { records: u32::MAX, ..base },
        Limits { objects: u32::MAX, ..base },
        Limits { notices: u32::MAX, ..base },
        Limits { rows: u32::MAX, ..base },
        // The timer's tick and the tree's final name must fit.
        Limits { notices: u32::MAX - base.objects - base.records, ..base },
        Limits { rows: 65_536, ..base },
    ];
    for (case, limits) in cases.into_iter().enumerate() {
        assert!(!valid(&limits), "case {case}");
    }
}

#[test]
fn validity_does_not_price_retained_documents() {
    let limits = crate::Limits { document_bytes: u64::MAX, ..super::domain::spine::LIMITS };
    assert!(crate::valid(&limits));
}

mod windows {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::windows::valid(&base));
        for (limits, expected) in
            [(crate::Limits { rows: u32::MAX / 2, ..base }, true), (crate::Limits { rows: u32::MAX, ..base }, false)]
        {
            assert_eq!(crate::windows::valid(&limits), expected);
        }
    }
}

mod objects {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::objects::valid(&base));
        for (limits, expected) in [
            (crate::Limits { objects: u32::MAX / 2, ..base }, true),
            (crate::Limits { objects: u32::MAX, ..base }, false),
        ] {
            assert_eq!(crate::objects::valid(&limits), expected);
        }
    }
}

mod notices_kept {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::notices::kept::valid(&base));
        for (limits, expected) in [
            (crate::Limits { notices: u32::MAX / 2, ..base }, true),
            (crate::Limits { notices: u32::MAX, ..base }, false),
        ] {
            assert_eq!(crate::notices::kept::valid(&limits), expected);
        }
    }
}

mod page_task {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::page::task::valid(&base));
        for (limits, expected) in
            [(crate::Limits { rows: u32::MAX - 1, ..base }, true), (crate::Limits { rows: u32::MAX, ..base }, false)]
        {
            assert_eq!(crate::page::task::valid(&limits), expected);
        }
    }
}

mod page_inbox {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::page::inbox::valid(&base));
        for (limits, expected) in
            [(crate::Limits { rows: u32::MAX / 4, ..base }, true), (crate::Limits { rows: u32::MAX, ..base }, false)]
        {
            assert_eq!(crate::page::inbox::valid(&limits), expected);
        }
    }
}

mod page_conversations {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::page::conversations::valid(&base));
        for (limits, expected) in
            [(crate::Limits { rows: u32::MAX / 3, ..base }, true), (crate::Limits { rows: u32::MAX, ..base }, false)]
        {
            assert_eq!(crate::page::conversations::valid(&limits), expected);
        }
    }
}

mod page_project {
    #[test]
    fn valid_refuses_each_unrepresentable_limit() {
        let base = super::super::domain::spine::LIMITS;
        assert!(crate::page::project::valid(&base));
        for (limits, expected) in
            [(crate::Limits { rows: u32::MAX / 2, ..base }, true), (crate::Limits { rows: u32::MAX, ..base }, false)]
        {
            assert_eq!(crate::page::project::valid(&limits), expected);
        }
    }
}
