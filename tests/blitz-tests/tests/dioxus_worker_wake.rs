use blitz_dom::Document;
use dioxus_core::{Element, ScopeId, VirtualDom, consume_context, spawn};
use dioxus_core_macro::rsx;
use dioxus_hooks::use_signal;
use dioxus_html as dioxus_elements;
use dioxus_native_dom::{DioxusDocument, DocumentConfig};
use dioxus_signals::{Signal, WritableExt};
use std::{
    cell::{Cell, RefCell},
    future::poll_fn,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

#[derive(Default)]
struct Gate {
    ready: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}
impl Gate {
    async fn wait(&self) {
        poll_fn(|cx| {
            if self.ready.get() {
                Poll::Ready(())
            } else {
                self.waker.replace(Some(cx.waker().clone()));
                Poll::Pending
            }
        })
        .await
    }
    fn release(&self) {
        self.ready.set(true);
        self.waker
            .borrow_mut()
            .take()
            .expect("task must be awaiting this gate")
            .wake();
    }
}

#[derive(Default)]
struct Probe {
    signal: RefCell<Option<Signal<usize>>>,
    first: Gate,
    second: Gate,
}
fn app() -> Element {
    let probe = consume_context::<Rc<Probe>>();
    let count = use_signal(|| 0usize);
    probe.signal.replace(Some(count));
    rsx! { p { "{count}" } }
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(doc: &mut DioxusDocument, waker: &Waker) -> bool {
    doc.poll(Some(Context::from_waker(waker)))
}
fn text(doc: &DioxusDocument) -> String {
    let inner = doc.inner.borrow();
    let mut stack = vec![inner.root_node().id];
    let mut text = String::new();
    while let Some(id) = stack.pop() {
        let node = inner.get_node(id).unwrap();
        if let Some(data) = node.text_data() {
            text.push_str(&data.content);
        }
        stack.extend(node.children.iter().rev().copied());
    }
    text
}

fn check_worker_wakes(extra_idle_poll: bool) {
    let probe = Rc::new(Probe::default());
    let vdom = VirtualDom::new(app).with_root_context(probe.clone());
    let mut doc = DioxusDocument::new(vdom, DocumentConfig::default());
    doc.initial_build();
    let wakes = Arc::new(WakeCount::default());
    let waker = Waker::from(wakes.clone());
    // Register the shell's wake-up destination while the document is idle.
    for _ in 0..4 {
        if !poll(&mut doc, &waker) {
            break;
        }
    }
    assert_eq!(text(&doc), "0");
    let mut count = probe.signal.borrow().unwrap();
    doc.vdom.in_scope(ScopeId::APP, || {
        count.set(1);
        spawn({
            let probe = probe.clone();
            async move {
                probe.first.wait().await;
                count.set(2);
                probe.second.wait().await;
                count.set(3);
            }
        });
    });
    assert!(poll(&mut doc, &waker));
    assert_eq!(text(&doc), "1");
    // A subsequent native redraw can incidentally cause this extra poll. The
    // first test deliberately requires Document::poll to be sufficient itself.
    if extra_idle_poll {
        assert!(!poll(&mut doc, &waker));
    }
    wakes.0.store(0, Ordering::SeqCst);
    probe.first.release();
    assert!(
        wakes.0.load(Ordering::SeqCst) > 0,
        "first awaited worker completion must wake the shell without a manual pump"
    );
    assert!(poll(&mut doc, &waker));
    assert_eq!(text(&doc), "2");
    if extra_idle_poll {
        assert!(!poll(&mut doc, &waker));
    }
    wakes.0.store(0, Ordering::SeqCst);
    probe.second.release();
    assert!(
        wakes.0.load(Ordering::SeqCst) > 0,
        "second awaited worker completion must also wake the shell"
    );
    assert!(poll(&mut doc, &waker));
    assert_eq!(text(&doc), "3");
    assert!(!poll(&mut doc, &waker));
}

#[test]
fn async_worker_wakes_after_a_single_document_poll() {
    check_worker_wakes(false);
}

#[test]
fn trailing_idle_poll_is_a_compensating_control() {
    check_worker_wakes(true);
}
