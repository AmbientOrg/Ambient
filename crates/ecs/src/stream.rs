use std::{
    collections::{HashMap, HashSet}, fmt::Display, sync::Arc
};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::{
    ArchetypeFilter, Component, ComponentUnit, ComponentValue, EntityData, EntityId, FramedEventsReader, IComponent, Query, QueryState, World
};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct WorldDiff {
    pub changes: Vec<WorldChange>,
}
impl WorldDiff {
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }
    pub fn set<T: ComponentValue>(self, id: EntityId, component: Component<T>, value: T) -> Self {
        self.set_unit(id, ComponentUnit::new(component, value))
    }
    pub fn add_component<T: ComponentValue>(self, id: EntityId, component: Component<T>, value: T) -> Self {
        self.add_unit(id, ComponentUnit::new(component, value))
    }
    pub fn remove_component(mut self, id: EntityId, component: &dyn IComponent) -> Self {
        self.changes.push(WorldChange::RemoveComponents(id, vec![component.clone_boxed()]));
        self
    }
    pub fn remove_components_raw(mut self, id: EntityId, components: Vec<Box<dyn IComponent>>) -> Self {
        self.changes.push(WorldChange::RemoveComponents(id, components));
        self
    }
    pub fn set_unit(mut self, id: EntityId, unit: ComponentUnit) -> Self {
        self.changes.push(WorldChange::Set(id, unit));
        self
    }
    pub fn add_unit(mut self, id: EntityId, unit: ComponentUnit) -> Self {
        let mut data = EntityData::new();
        data.set_unit(unit);
        self.changes.push(WorldChange::AddComponents(id, data));
        self
    }
    pub fn despawn(mut self, ids: Vec<EntityId>) -> Self {
        self.changes.extend(ids.into_iter().map(WorldChange::Despawn));
        self
    }
    pub fn apply(self, world: &mut World, spanwed_extra_data: EntityData, create_revert: bool) -> Option<Self> {
        let revert_changes =
            self.changes.into_iter().map(|change| change.apply(world, &spanwed_extra_data, false, create_revert)).collect_vec();
        if create_revert {
            Some(Self { changes: revert_changes.into_iter().rev().flatten().collect_vec() })
        } else {
            None
        }
    }
    pub fn is_empty(&self) -> bool {
        self.changes.len() == 0
    }
    /// This creates a list of changes that would take you from the `from` world to the `to` world, if applied to the `from` world.
    pub fn from_a_to_b(filter: WorldStreamFilter, from: &World, to: &World) -> Self {
        let from_entities: HashSet<EntityId> = filter.all_entities(from).collect();
        let to_entities: HashSet<EntityId> = filter.all_entities(to).collect();
        let spawned = to_entities.iter().filter(|id| !from_entities.contains(id)).cloned().collect_vec();
        let despawned = from_entities.iter().filter(|id| !to_entities.contains(id)).cloned().collect_vec();
        let in_both = to_entities.iter().filter(|id| from_entities.contains(id)).cloned().collect_vec();

        let spawned = spawned.into_iter().map(|id| WorldChange::Spawn(Some(id), filter.read_entity_components(to, id).into()));
        let despanwed = despawned.into_iter().map(|id| WorldChange::Despawn(id));
        let updated = in_both.into_iter().flat_map(|id| {
            let from_comps: HashSet<Box<dyn IComponent>> = filter.get_entity_components(from, id).into_iter().collect();
            let to_comps: HashSet<Box<dyn IComponent>> = filter.get_entity_components(to, id).into_iter().collect();
            let added = to_comps.iter().filter(|c| !from_comps.contains(*c)).collect_vec();
            let removed = from_comps.iter().filter(|c| !to_comps.contains(*c)).cloned().collect_vec();
            let in_both = to_comps.iter().filter(|c| from_comps.contains(*c)).cloned().collect_vec();
            let changed = in_both
                .into_iter()
                .filter(|c| {
                    from.get_component_content_version(id, c.as_ref()).unwrap() != to.get_component_content_version(id, c.as_ref()).unwrap()
                })
                .collect_vec();
            let added: EntityData = added
                .into_iter()
                .map(|comp| ComponentUnit::new_raw(comp.clone_boxed(), comp.clone_value_from_world(to, id).unwrap()))
                .collect();
            let added = if !added.is_empty() { vec![WorldChange::AddComponents(id, added)] } else { vec![] };
            let removed = if !removed.is_empty() { vec![WorldChange::RemoveComponents(id, removed)] } else { vec![] };
            let changed = changed
                .into_iter()
                .map(|comp| {
                    let unit = ComponentUnit::new_raw(comp.clone_boxed(), comp.clone_value_from_world(to, id).unwrap());
                    WorldChange::Set(id, unit)
                })
                .collect_vec();
            added.into_iter().chain(removed.into_iter()).chain(changed.into_iter())
        });

        Self { changes: despanwed.chain(spawned).chain(updated).collect_vec() }
    }
}
impl Display for WorldDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for change in self.changes.iter().take(3) {
            change.fmt(f)?;
            write!(f, " ").unwrap();
        }
        if self.changes.len() > 3 {
            write!(f, "...{} more", self.changes.len() - 3).unwrap();
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldStreamCompEvent {
    Init,
    Set,
    Spawn,
    AddComponent,
    RemoveComponent,
}

#[derive(Clone)]
pub struct WorldStreamFilter {
    arch_filter: ArchetypeFilter,
    component_filter: Arc<dyn Fn(&dyn IComponent, WorldStreamCompEvent) -> bool + Sync + Send>,
}
impl WorldStreamFilter {
    pub fn new(
        arch_filter: ArchetypeFilter,
        component_filter: Arc<dyn Fn(&dyn IComponent, WorldStreamCompEvent) -> bool + Sync + Send>,
    ) -> Self {
        Self { arch_filter, component_filter }
    }
    pub fn initial_diff(&self, world: &World) -> WorldDiff {
        WorldDiff {
            changes: self
                .all_entities(world)
                .map(|id| WorldChange::Spawn(Some(id), self.read_entity_components(world, id).into()))
                .collect_vec(),
        }
    }
    pub fn all_entities<'a>(&self, world: &'a World) -> impl Iterator<Item = EntityId> + 'a {
        Query::all().filter(&self.arch_filter).iter(world, None).map(|x| x.id())
    }
    pub fn get_entity_components(&self, world: &World, id: EntityId) -> Vec<Box<dyn IComponent>> {
        world
            .get_components(id)
            .unwrap()
            .into_iter()
            .filter(|comp| (self.component_filter)(comp.as_ref(), WorldStreamCompEvent::Init))
            .collect_vec()
    }
    fn read_entity_components(&self, world: &World, id: EntityId) -> Vec<ComponentUnit> {
        let components = self
            .get_entity_components(world, id)
            .into_iter()
            .map(|comp| ComponentUnit::new_raw(comp.clone_boxed(), comp.clone_value_from_world(world, id).unwrap()))
            .collect_vec();
        components
    }
}
impl Default for WorldStreamFilter {
    fn default() -> Self {
        Self { arch_filter: Default::default(), component_filter: Arc::new(|_, _| true) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WorldChange {
    Spawn(Option<EntityId>, EntityData),
    Despawn(EntityId),
    AddComponents(EntityId, EntityData),
    RemoveComponents(EntityId, Vec<Box<dyn IComponent>>),
    Set(EntityId, ComponentUnit),
}
impl WorldChange {
    pub fn is_set(&self) -> bool {
        matches!(self, Self::Set(_, _))
    }
    pub fn is_remove_components(&self) -> bool {
        matches!(self, Self::RemoveComponents(_, _))
    }
    fn apply(self, world: &mut World, spanwed_extra_data: &EntityData, panic_on_error: bool, create_revert: bool) -> Option<Self> {
        match self {
            Self::Spawn(id, data) => {
                if let Some(id) = id {
                    if !world.spawn_mirrored(id, data.append(spanwed_extra_data.clone())) {
                        if panic_on_error {
                            panic!("WorldChange::apply spawn_mirror entity already exists: {:?}", id);
                        } else {
                            log::error!("WorldChange::apply spawn_mirror entity already exists: {:?}", id);
                        }
                    }
                    if create_revert {
                        return Some(Self::Despawn(id));
                    }
                } else {
                    let id = world.spawn(data.append(spanwed_extra_data.clone()));
                    if create_revert {
                        return Some(Self::Despawn(id));
                    }
                }
            }
            Self::Despawn(id) => {
                let res = if create_revert {
                    world.get_components(id).ok().map(|components| {
                        let mut ed = EntityData::new();
                        for comp in components {
                            // Only serializable components
                            if comp.is_extended() {
                                ed.set_raw(comp.clone(), comp.clone_value_from_world(world, id).unwrap());
                            }
                        }
                        Self::Spawn(Some(id), ed)
                    })
                } else {
                    None
                };
                world.despawn(id);
                return res;
            }
            Self::AddComponents(id, data) => {
                let res = if create_revert { Some(Self::RemoveComponents(id, data.components())) } else { None };
                world.add_components(id, data).unwrap();
                return res;
            }
            Self::RemoveComponents(id, comps) => {
                let res = if create_revert {
                    Some(Self::AddComponents(id, comps.iter().filter_map(|comp| world.get_unit(id, comp.as_ref()).ok()).collect()))
                } else {
                    None
                };
                for comp in comps {
                    world.remove_component(id, comp).unwrap();
                }
                return res;
            }
            Self::Set(id, unit) => {
                let prev = match unit.set_at_entity(world, id) {
                    Ok(unit) => unit,
                    Err(err) => {
                        if panic_on_error {
                            panic!("Failed to set: {:?}", err);
                        } else {
                            return None;
                        }
                    }
                };
                if create_revert {
                    return Some(Self::Set(id, prev));
                }
            }
        }
        None
    }
    fn filter(&self, world: &World, filter: &WorldStreamFilter) -> Option<Self> {
        match self {
            Self::Spawn(id, data) => {
                if !filter.arch_filter.matches_entity(world, (*id).unwrap()) {
                    return None;
                }
                let mut data = data.clone();
                data.filter(&|comp| (filter.component_filter)(comp, WorldStreamCompEvent::Spawn));
                Some(Self::Spawn(*id, data.clone()))
            }
            Self::Despawn(id) => {
                // TODO: Right now we don't filter despawns, because the spawns are filtered, so the "bad" despawns will just be
                // ignored on the client side. Maybe should filter them on the server side too
                Some(Self::Despawn(*id))
            }
            Self::AddComponents(id, data) => {
                if !filter.arch_filter.matches_entity(world, *id) {
                    return None;
                }
                let mut data = data.clone();
                data.filter(&|comp| (filter.component_filter)(comp, WorldStreamCompEvent::AddComponent));
                Some(Self::AddComponents(*id, data.clone()))
            }
            Self::RemoveComponents(id, comps) => {
                if !filter.arch_filter.matches_entity(world, *id) {
                    return None;
                }
                Some(Self::RemoveComponents(
                    *id,
                    comps
                        .iter()
                        .filter_map(|comp| {
                            if (filter.component_filter)(comp.as_ref(), WorldStreamCompEvent::RemoveComponent) {
                                Some(comp.clone_boxed())
                            } else {
                                None
                            }
                        })
                        .collect_vec(),
                ))
            }
            Self::Set(_id, _unit) => Some(self.clone()),
        }
    }
}
impl Display for WorldChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldChange::Spawn(id, data) => write!(f, "spawn({}, {})", id.unwrap_or(EntityId::null()), data.len()),
            WorldChange::Despawn(id) => write!(f, "despawn({id})"),
            WorldChange::AddComponents(id, data) => write!(f, "add_components({id}, {data:?})"),
            WorldChange::RemoveComponents(id, _) => write!(f, "remove_components({id})"),
            WorldChange::Set(id, data) => write!(f, "set({id}, {data:?})"),
        }
    }
}

#[derive(Clone)]
pub struct WorldStream {
    changed_qs: QueryState,
    shape_stream_reader: FramedEventsReader<WorldChange>,
    filter: WorldStreamFilter,
    version: u64,
}
impl WorldStream {
    pub fn new(filter: WorldStreamFilter) -> Self {
        Self { changed_qs: QueryState::new(), shape_stream_reader: FramedEventsReader::new(), filter, version: 0 }
    }
    pub fn filter(&self) -> &WorldStreamFilter {
        &self.filter
    }
    #[profiling::function]
    pub fn next_diff(&mut self, world: &World) -> WorldDiff {
        let shape_changes = self
            .shape_stream_reader
            .iter(world.shape_change_events.as_ref().unwrap())
            .filter_map(|(_, change)| change.filter(world, &self.filter))
            .collect_vec();

        let mut sets = HashMap::new();
        for arch in world.archetypes.iter() {
            if self.filter.arch_filter.matches(&arch.active_components) {
                for arch_comp in arch.components.iter() {
                    if (self.filter.component_filter)(arch_comp.component.as_ref(), WorldStreamCompEvent::Set) {
                        let reader = self.changed_qs.get_change_reader(arch.id, arch_comp.component.get_index());
                        for (_, entity_id) in reader.iter(&*arch_comp.changes.borrow()) {
                            if let Some(loc) = world.entity_loc(*entity_id) {
                                if loc.archetype == arch.id && arch_comp.get_content_version(loc.index) > self.version {
                                    let entry = sets.entry(*entity_id).or_insert_with(Vec::new);
                                    entry.push(ComponentUnit::new_raw(
                                        arch_comp.component.clone_boxed(),
                                        arch_comp.component.clone_value_from_world(world, *entity_id).unwrap(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        self.version = world.version();
        let mut changes = shape_changes;
        changes.extend(sets.into_iter().flat_map(|(id, units)| units.into_iter().map(move |unit| WorldChange::Set(id, unit))));
        WorldDiff { changes }
    }
}
