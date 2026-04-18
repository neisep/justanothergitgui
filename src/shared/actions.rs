use crate::app::TabActionContext;

pub trait HandleUiAction {
    fn apply(self: Box<Self>, ctx: &mut TabActionContext<'_>);
}

pub struct UiAction(Box<dyn HandleUiAction>);

impl UiAction {
    pub(crate) fn new(action: impl HandleUiAction + 'static) -> Self {
        Self(Box::new(action))
    }

    pub(crate) fn apply(self, ctx: &mut TabActionContext<'_>) {
        self.0.apply(ctx);
    }
}
