use super::*;
use nepl3_core::{
    budget::Limits,
    source::{Digest, SourceId, SourceRef},
};

#[test]
fn admission_exchange_is_sticky_even_after_restoration_and_errors() -> Result<(), ReaderError> {
    struct Exchange {
        saved: SourceAdmission,
        fail: bool,
        restore_inside: bool,
    }
    impl TokenizationHost for Exchange {
        fn provider(
            &mut self,
            _: &ProviderCall,
            _: &mut Budget,
            _: &mut SourceAdmission,
        ) -> Result<Option<ProviderReply>, ReaderError> {
            Err(ReaderError::ProviderContract)
        }
        fn reservation(
            &mut self,
            _: &ReservationRequest,
            _: &mut Budget,
            admission: &mut SourceAdmission,
        ) -> Result<Option<SourceReservation>, ReaderError> {
            core::mem::swap(&mut self.saved, admission);
            if self.restore_inside {
                core::mem::swap(&mut self.saved, admission);
            }
            if self.fail {
                Err(ReaderError::ProviderContract)
            } else {
                Ok(None)
            }
        }
    }
    let request = ReservationRequest {
        session_id: "s".into(),
        request_id: 0,
        snapshot: SourceRef {
            source_id: SourceId("source".into()),
            revision: 0,
            digest: Digest([0; 32]),
        },
        start: 0,
        limit: 1,
    };
    for fail in [false, true] {
        for restore_inside in [false, true] {
            let mut budget = Budget::new(Limits {
                source_bytes: 100,
                work: 100,
                depth: 10,
                nodes: 100,
                allocation_units: 1000,
                output_bytes: 100,
                diagnostics: 10,
                events: 10,
            });
            let mut admission = SourceAdmission::default();
            let scope = admission.scope_with_budget(&mut budget)?;
            let mut exchange = Exchange {
                saved: SourceAdmission::default(),
                fail,
                restore_inside,
            };
            let mut host = AdmissionHost {
                inner: &mut exchange,
                scope: scope.as_ref(),
                changed: false,
            };
            for _ in 0..2 {
                let result = host.reservation(&request, &mut budget, &mut admission);
                assert_eq!(result.is_err(), fail);
                assert_eq!(
                    host.changed,
                    !restore_inside || !cfg!(target_has_atomic = "ptr")
                );
            }
            // Two separate exchanges restore A, but must not restore its proof.
            if let Some(scope) = scope.as_ref() {
                assert!(admission.matches_scope(scope));
            }
        }
    }
    Ok(())
}
