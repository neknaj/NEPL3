        if let Action::Closure { order, mode } = self.action {
            for at in 0..32 {
                let i = match order { 0 => at, 1 => 31-at, _ => (at*13)%32 };
                let name = format!("aux:{}:{:04}", "\u{65e5}\u{672c}".repeat(8), i/2);
                let uri = if mode == 1 && self.calls == 2 && at == 15 { "changed:uri".into() } else { format!("memory:aux:{i}") };
                // Independent owned snapshot can repeat prior identity; admission and
                // actual reader boundary must reject the changed URI on call two.
                let revision = if i%2==0 { 0 } else { u64::MAX };
                let source = if let Some(existing) = request.sources.iter().find(|s|s.identity().source.0==name && s.identity().revision==revision && s.uri()==uri) {
                    existing.clone_with_budget(budget)?
                } else if uri == "changed:uri" {
                    nepl3_core::source::SourceSnapshot::new(SourceId(name), revision, uri, b"abc".to_vec(), budget)?
                } else {
                    admission.create(SourceId(name), revision, uri, b"abc".to_vec(), budget)?
                };
                generated.push(source);
            }
        }
