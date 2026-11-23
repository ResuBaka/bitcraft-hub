// Test fixture only. Use the generated context and the normal apply/dispatch path.
impl<M: SpacetimeModule> DbContextImpl<M> {
    pub fn replay_context() -> Self {
        let (rt, handle) = enter_or_create_runtime().unwrap();
        let (pending, recv) = mpsc::unbounded();
        build_db_ctx(
            handle,
            build_db_ctx_inner(rt, None, None, None),
            mpsc::unbounded().0,
            Arc::new(TokioMutex::new(mpsc::unbounded().1)),
            pending,
            Arc::new(TokioMutex::new(recv)),
            None,
            true,
            None,
        )
    }
    pub fn replay_register(&self) {
        self.apply_pending_mutations().unwrap();
    }
    pub fn replay_tx(&self, update: M::DbUpdate) {
        self.apply_update(update, |_| Event::Transaction);
    }
}
