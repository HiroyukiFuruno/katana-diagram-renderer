use super::page::ChromiumPage;
use std::cell::RefCell;

thread_local! {
    static PAGE: RefCell<Option<ChromiumPage>> = const { RefCell::new(None) };
}

pub(super) fn with_page<T>(operation: impl FnOnce(&RefCell<Option<ChromiumPage>>) -> T) -> T {
    PAGE.with(operation)
}

pub(super) fn clear() {
    with_page(|slot| *slot.borrow_mut() = None);
}
