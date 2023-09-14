//! gflux is a reactive component system designed to make GTK more manageable

#![allow(clippy::type_complexity)]
#![warn(rustdoc::all)]
#![warn(missing_debug_implementations)]

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::rc::{Rc, Weak};

mod rev_track;
pub use rev_track::*;

mod obs;
pub use obs::*;

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
    global: Rc<RefCell<Obs<M>>>,
    comp_table: Rc<RefCell<ComponentTable>>,
    roots: Rc<RefCell<BTreeSet<ComponentId>>>,
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
    pub fn new(global: Rc<RefCell<Obs<M>>>) -> Self {
        Self {
            global,
            comp_table: Rc::new(RefCell::new(ComponentTable::new())),
            roots: Rc::new(RefCell::new(BTreeSet::new())),
        }
    }

    /// Execute rebuild on every dirty component, and their ancestors, from the top down.
    pub fn exec_rebuilds(&self) {
        // clean up dead components from root list
        self.roots.borrow_mut().retain(|cid| {
            self.comp_table
                .borrow()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade())
                .is_some()
        });

        // Execute rebuild on all roots
        for cid in &*self.roots.borrow() {
            let c = self
                .comp_table
                .borrow()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade());
            if let Some(c) = c {
                dbg!(cid);
                c.borrow_mut().rebuild();
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
        C: Component<GlobalModel = M> + 'static,
        L: Fn(&C::GlobalModel) -> &C::Model + 'static,
        LM: Fn(&mut C::GlobalModel) -> &mut C::Model + 'static,
    {
        let id = self.comp_table.borrow_mut().reserve_id();
        let mut ctx = ComponentCtx {
            global: self.global.clone(),
            comp_table: self.comp_table.clone(),
            id,
            parent_id: None,
            children: Rc::new(RefCell::new(BTreeSet::new())),
            lens: Rc::new(lens),
            lens_mut: Rc::new(lens_mut),
        };

        let mut component = C::build(ctx.clone(), params);
        component.rebuild(ctx.clone());
        let c = Rc::new(RefCell::new(ComponentBase {
            ctx: ctx.clone(),
            component,
        }));

        ctx.id = ctx
            .comp_table
            .borrow_mut()
            .insert(id, Rc::downgrade(&c) as WeakComponentBase);

        self.roots.borrow_mut().insert(id);

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
pub struct ComponentHandle<C: Component> {
    inner: Rc<RefCell<ComponentBase<C>>>,
}

impl<C: Component> ComponentHandle<C> {
    /// Returns the root widget
    pub fn widget(&self) -> C::Widget {
        self.inner.borrow().component.widget()
    }

    /// Rebuilds the component.  You shouldn't need to call this manually if
    /// you've mutated this component's state using its `ComponentCtx`.
    pub fn rebuild(&self) {
        self.inner.borrow_mut().rebuild()
    }
}

#[derive(Debug)]
struct ComponentBase<C: Component> {
    ctx: ComponentCtx<C>,
    component: C,
}

impl<C: Component> ComponentBaseTrait for ComponentBase<C> {
    fn id(&self) -> ComponentId {
        self.ctx.id
    }
    fn parent_id(&self) -> Option<ComponentId> {
        self.ctx.parent_id
    }
    fn rebuild(&mut self) {
        self.component.rebuild(self.ctx.clone());
    }
}

impl<C: Component> Drop for ComponentBase<C> {
    fn drop(&mut self) {
        self.ctx.comp_table.borrow_mut().destroy(self.ctx.id);
    }
}

trait ComponentBaseTrait {
    fn id(&self) -> ComponentId;
    fn parent_id(&self) -> Option<ComponentId>;
    fn rebuild(&mut self);
}

type ComponentId = u64;
type WeakComponentBase = Weak<RefCell<dyn ComponentBaseTrait>>;

/// Performs bookkeeping for the component, and provides state accessor methods
pub struct ComponentCtx<C: Component + ?Sized> {
    id: ComponentId,
    parent_id: Option<ComponentId>,
    children: Rc<RefCell<BTreeSet<ComponentId>>>,

    global: Rc<RefCell<Obs<C::GlobalModel>>>,
    comp_table: Rc<RefCell<ComponentTable>>,
    lens: Rc<dyn Fn(&C::GlobalModel) -> &C::Model>,
    lens_mut: Rc<dyn Fn(&mut C::GlobalModel) -> &mut C::Model>,
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
    pub fn create_child<K: Component<GlobalModel = C::GlobalModel> + 'static, L, LM>(
        &self,
        p_to_c: L,
        p_to_c_mut: LM,
        params: K::Params,
    ) -> ComponentHandle<K>
    where
        L: Fn(&C::Model) -> &K::Model + 'static,
        LM: Fn(&mut C::Model) -> &mut K::Model + 'static,
        C::Model: 'static,
        C::GlobalModel: 'static,
        K::Model: 'static,
        K::GlobalModel: 'static,
    {
        let p_lens = self.lens.clone();
        let child_lens: Rc<dyn Fn(&C::GlobalModel) -> &K::Model> =
            Rc::new(move |g| p_to_c(p_lens(g)));

        let p_lens_mut = self.lens_mut.clone();
        let child_lens_mut: Rc<dyn Fn(&mut C::GlobalModel) -> &mut K::Model> =
            Rc::new(move |g| p_to_c_mut(p_lens_mut(g)));

        let id = self.comp_table.borrow_mut().reserve_id();
        let mut ctx = ComponentCtx {
            id,
            parent_id: Some(self.id),
            children: Rc::new(RefCell::new(BTreeSet::new())),
            comp_table: self.comp_table.clone(),
            global: self.global.clone(),
            lens: child_lens,
            lens_mut: child_lens_mut,
        };
        let mut component = K::build(ctx.clone(), params);
        component.rebuild(ctx.clone());
        let c = Rc::new(RefCell::new(ComponentBase {
            ctx: ctx.clone(),
            component,
        }));

        ctx.id = ctx
            .comp_table
            .borrow_mut()
            .insert(id, Rc::downgrade(&c) as WeakComponentBase);

        self.children.borrow_mut().insert(id);

        ComponentHandle { inner: c }
    }

    pub fn rebuild_children(&self) {
        // Clean up dead components from root list
        self.children.borrow_mut().retain(|cid| {
            self.comp_table
                .borrow()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade())
                .is_some()
        });

        // Execute rebuild, then rebuild_children
        for cid in &*self.children.borrow() {
            let c = self
                .comp_table
                .borrow()
                .map
                .get(cid)
                .and_then(|wr| wr.upgrade());
            if let Some(c) = c {
                c.borrow_mut().rebuild();
            }
        }
    }

    /// Access the component state
    pub fn with_model<R, F: Fn(&C::Model) -> R>(&self, f: F) -> R {
        let global = self.global.borrow();
        let lens = self.lens.clone();
        f(lens(global.get()))
    }

    /// Access the component state mutably, marks the component as dirty.
    pub fn with_model_mut<R, F: Fn(&mut C::Model) -> R>(&self, f: F) -> R {
        let mut global = self.global.borrow_mut();
        let lens_mut = self.lens_mut.clone();
        f(lens_mut(global.get_mut()))
    }
}
