//! gflux is a reactive component system designed to make GTK more manageable

#![allow(clippy::type_complexity)]
#![warn(rustdoc::all)]
#![warn(missing_debug_implementations)]

mod obs;
pub use obs::*;

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, RwLock, Weak};

/// The trait that defines a component
pub trait Component {
    /// The global application state
    type GlobalModel;
    /// The application state for this component
    type Model;
    /// The root widget type for this component
    type Widget;
    /// The parameters needed to build this component
    type Params;

    /// Returns the root widget
    fn widget(&self) -> Self::Widget;
    /// Builds the component
    fn build(ctx: ComponentCtx<Self>, params: Self::Params) -> Self;
    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        ctx.rebuild_children();
    }
}

/// Manages the component tree
pub struct ComponentTree<M> {
    global: Arc<RwLock<Obs<M>>>,
    comp_table: Arc<RwLock<ComponentTable>>,
    roots: Arc<RwLock<BTreeSet<ComponentId>>>,
}

impl<M: fmt::Debug> fmt::Debug for ComponentTree<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentTree")
            .field("global", &self.global)
            .field("comp_table", &self.comp_table)
            .finish()
    }
}

impl<M> Clone for ComponentTree<M> {
    fn clone(&self) -> Self {
        Self {
            global: self.global.clone(),
            comp_table: self.comp_table.clone(),
            roots: self.roots.clone(),
        }
    }
}

impl<M> ComponentTree<M> {
    pub fn new(global: Arc<RwLock<Obs<M>>>) -> Self {
        Self {
            global,
            comp_table: Arc::new(RwLock::new(ComponentTable::new())),
            roots: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }

    /// Execute rebuild on every dirty component, and their ancestors, from the top down.
    pub fn exec_rebuilds(&self) {
        // clean up dead components from root list
        self.roots.write().unwrap().retain(|cid| {
            self.comp_table
                .read()
                .unwrap()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade())
                .is_some()
        });

        // Execute rebuild on all roots
        for cid in &*self.roots.read().unwrap() {
            let c = self
                .comp_table
                .read()
                .unwrap()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade());
            if let Some(c) = c {
                c.write().unwrap().rebuild();
            }
        }
    }

    /// Create a new root component
    pub fn new_component<L, LM, C>(
        &mut self,
        lens: L,
        lens_mut: LM,
        params: C::Params,
    ) -> ComponentHandle<C>
    where
        C: Component<GlobalModel = M> + Send + Sync + 'static,
        L: Fn(&C::GlobalModel) -> &C::Model + Send + Sync + 'static,
        LM: Fn(&mut C::GlobalModel) -> &mut C::Model + Send + Sync + 'static,
        C::Model: Send + Sync,
        M: Send + Sync,
    {
        let id = self.comp_table.write().unwrap().reserve_id();
        let mut ctx = ComponentCtx {
            global: self.global.clone(),
            comp_table: self.comp_table.clone(),
            id,
            parent_id: None,
            children: Arc::new(RwLock::new(BTreeSet::new())),
            lens: Arc::new(lens),
            lens_mut: Arc::new(lens_mut),
        };

        let mut component = C::build(ctx.clone(), params);
        component.rebuild(ctx.clone());
        let c = Arc::new(RwLock::new(ComponentBase {
            ctx: ctx.clone(),
            component,
        }));

        ctx.id = ctx
            .comp_table
            .write()
            .unwrap()
            .insert(id, Arc::downgrade(&c) as WeakComponentBase);

        self.roots.write().unwrap().insert(id);

        ComponentHandle { inner: c }
    }
}

#[derive(Debug)]
struct ComponentTable {
    pub next_id: ComponentId,
    pub map: HashMap<ComponentId, WeakComponentBase>,
}

impl ComponentTable {
    fn new() -> Self {
        Self {
            next_id: 1,
            map: HashMap::new(),
        }
    }

    fn reserve_id(&mut self) -> ComponentId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn insert(&mut self, cid: ComponentId, c: WeakComponentBase) -> ComponentId {
        self.map.insert(cid, c);
        cid
    }

    fn destroy(&mut self, cid: ComponentId) {
        self.map.remove(&cid);
    }
}

/// Handle for a component
#[derive(Debug)]
pub struct ComponentHandle<C: Component + Send + Sync>
where
    C::GlobalModel: Send + Sync,
    C::Model: Send + Sync,
{
    inner: Arc<RwLock<ComponentBase<C>>>,
}

impl<C: Component + Send + Sync> ComponentHandle<C>
where
    C::GlobalModel: Send + Sync,
    C::Model: Send + Sync,
{
    /// Returns the root widget
    pub fn widget(&self) -> C::Widget {
        self.inner.read().unwrap().component.widget()
    }

    /// Rebuilds the component.  You shouldn't need to call this manually if
    /// you've mutated this component's state using its `ComponentCtx`.
    pub fn rebuild(&self) {
        self.inner.write().unwrap().rebuild()
    }
}

#[derive(Debug)]
struct ComponentBase<C: Component + Send + Sync>
where
    C::GlobalModel: Send + Sync,
    C::Model: Send + Sync,
{
    ctx: ComponentCtx<C>,
    component: C,
}

impl<C: Component + Send + Sync> ComponentBaseTrait for ComponentBase<C>
where
    C::GlobalModel: Send + Sync,
    C::Model: Send + Sync,
{
    fn rebuild(&mut self) {
        self.component.rebuild(self.ctx.clone());
    }
}

impl<C: Component + Send + Sync> Drop for ComponentBase<C>
where
    C::GlobalModel: Send + Sync,
    C::Model: Send + Sync,
{
    fn drop(&mut self) {
        self.ctx.comp_table.write().unwrap().destroy(self.ctx.id);
    }
}

trait ComponentBaseTrait: Send + Sync {
    fn rebuild(&mut self);
}

type ComponentId = u64;
type WeakComponentBase = Weak<RwLock<dyn ComponentBaseTrait>>;

/// Performs bookkeeping for the component, and provides state accessor methods
pub struct ComponentCtx<C: Component + ?Sized> {
    id: ComponentId,
    parent_id: Option<ComponentId>,
    children: Arc<RwLock<BTreeSet<ComponentId>>>,

    global: Arc<RwLock<Obs<C::GlobalModel>>>,
    comp_table: Arc<RwLock<ComponentTable>>,
    lens: Arc<dyn Fn(&C::GlobalModel) -> &C::Model + Send + Sync>,
    lens_mut: Arc<dyn Fn(&mut C::GlobalModel) -> &mut C::Model + Send + Sync>,
}

impl<C: Component + fmt::Debug> fmt::Debug for ComponentCtx<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentTree")
            .field("id", &self.id)
            .field("parent_id", &self.parent_id)
            .field("comp_table", &self.comp_table)
            .finish()
    }
}

impl<C: Component + ?Sized> Clone for ComponentCtx<C> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            parent_id: self.parent_id,
            children: self.children.clone(),
            global: self.global.clone(),
            comp_table: self.comp_table.clone(),
            lens: self.lens.clone(),
            lens_mut: self.lens_mut.clone(),
        }
    }
}

impl<C: Component + ?Sized> ComponentCtx<C> {
    /// Creates a component that is a child of this component
    pub fn create_child<K: Component<GlobalModel = C::GlobalModel> + Send + Sync + 'static, L, LM>(
        &self,
        p_to_c: L,
        p_to_c_mut: LM,
        params: K::Params,
    ) -> ComponentHandle<K>
    where
        L: Fn(&C::Model) -> &K::Model + Send + Sync + 'static,
        LM: Fn(&mut C::Model) -> &mut K::Model + Send + Sync + 'static,
        C::Model: Send + Sync + 'static,
        C::GlobalModel: Send + Sync + 'static,
        K::Model: Send + Sync + 'static,
        K::GlobalModel: Send + Sync + 'static,
    {
        let p_lens = self.lens.clone();
        let child_lens: Arc<dyn Fn(&C::GlobalModel) -> &K::Model + Send + Sync> =
            Arc::new(move |g| p_to_c(p_lens(g)));

        let p_lens_mut = self.lens_mut.clone();
        let child_lens_mut: Arc<dyn Fn(&mut C::GlobalModel) -> &mut K::Model + Send + Sync> =
            Arc::new(move |g| p_to_c_mut(p_lens_mut(g)));

        let id = self.comp_table.write().unwrap().reserve_id();
        let mut ctx = ComponentCtx {
            id,
            parent_id: Some(self.id),
            children: Arc::new(RwLock::new(BTreeSet::new())),
            comp_table: self.comp_table.clone(),
            global: self.global.clone(),
            lens: child_lens,
            lens_mut: child_lens_mut,
        };
        let mut component = K::build(ctx.clone(), params);
        component.rebuild(ctx.clone());
        let c = Arc::new(RwLock::new(ComponentBase {
            ctx: ctx.clone(),
            component,
        }));

        ctx.id = ctx
            .comp_table
            .write()
            .unwrap()
            .insert(id, Arc::downgrade(&c) as WeakComponentBase);

        self.children.write().unwrap().insert(id);

        ComponentHandle { inner: c }
    }

    pub fn rebuild_children(&self) {
        // Clean up dead components from root list
        self.children.write().unwrap().retain(|cid| {
            self.comp_table
                .read()
                .unwrap()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade())
                .is_some()
        });

        // Execute rebuild, then rebuild_children
        for cid in &*self.children.read().unwrap() {
            let c = self
                .comp_table
                .read()
                .unwrap()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade());
            if let Some(c) = c {
                c.write().unwrap().rebuild();
            }
        }
    }

    /// Access the component state
    pub fn with_model<R, F: Fn(&C::Model) -> R>(&self, f: F) -> R {
        let global = self.global.read().unwrap();
        let lens = self.lens.clone();
        f(lens(global.get()))
    }

    /// Access the component state mutably, marks the component as dirty.
    pub fn with_model_mut<R, F: Fn(&mut C::Model) -> R>(&self, f: F) -> R {
        let mut global = self.global.write().unwrap();
        let lens_mut = self.lens_mut.clone();
        f(lens_mut(global.get_mut()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestModel {
        _data: i32,
    }

    #[derive(Debug)]
    struct TestComponent {
        value: i32,
    }

    impl Component for TestComponent {
        type GlobalModel = TestModel;
        type Model = i32;
        type Widget = i32;
        type Params = i32;

        fn widget(&self) -> Self::Widget {
            self.value
        }

        fn build(_ctx: ComponentCtx<Self>, params: Self::Params) -> Self {
            Self { value: params }
        }
    }

    #[test]
    fn test_component_tree_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send::<ComponentTree<TestModel>>();
        assert_sync::<ComponentTree<TestModel>>();
        assert_send_sync::<ComponentTree<TestModel>>();
    }
}
