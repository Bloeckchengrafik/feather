use base::{Area, AxisXyz, BlockId, Face, FacingCardinal, FacingCardinalAndDown, Gamemode, HalfTopBottom, Inventory, Item, ItemStack, SlabKind};
use blocks::BlockKind;
use ecs::{SysResult, SystemExecutor};
use libcraft_core::BlockFace;
use quill_common::events::BlockPlacementEvent;
use crate::chunk::entities::ChunkEntities;
use crate::entities::player::HotbarSlot;
use crate::events::BlockChangeEvent;
use crate::{Game, World};

pub fn register(systems: &mut SystemExecutor<Game>) {
    systems.add_system(block_placement);
}

pub fn block_placement(game: &mut Game) -> SysResult {
    let mut events = vec![];
    for (player, event) in game.ecs.query::<&BlockPlacementEvent>().iter() {
        let inv = game.ecs.get_mut::<Inventory>(player)?;
        let gamemode = game.ecs.get::<Gamemode>(player)?;
        let hotbar = game.ecs.get::<HotbarSlot>(player)?;
        let mut slot = match event.hand {
            libcraft_core::Hand::Main => inv.item(Area::Hotbar, hotbar.get()),
            libcraft_core::Hand::Offhand => inv.item(Area::Offhand, 0),
        }.unwrap();
        if slot.is_none() { continue; }
        let block = match item_to_block(slot.as_ref().unwrap().item()) {
            Some(s) => s,
            None => continue,
        };
        match place_block(&mut game.world, &game.chunk_entities, block, event) {
            Some(s) =>  {
                match *gamemode {
                    Gamemode::Survival | Gamemode::Adventure => decrease_slot(&mut slot),
                    Gamemode::Creative | Gamemode::Spectator => {},
                }
                events.push(s)
            },
            None => {},
        }
    }
    for e in events {
        game.ecs.insert_event(e);
    }
    
    Ok(())
}
/// Blocks get changed if they are getting waterlogged or when they are slabs turning into full blocks. 
/// Blocks get replaced in-place when possible, while changing an adjacent block has a lower priority.
fn place_block(world: &mut World, chunk_entities: &ChunkEntities, block: BlockId, placement: &BlockPlacementEvent) -> Option<BlockChangeEvent> {
    let target1 = placement.location;
    let target_block1 = world.block_at(target1)?;
    if target_block1.is_air() { return None; }
    let mut block = block;
    set_face(&mut block, placement);
    let target = if slab_to_place(&mut block, target_block1, placement) | waterlog(&mut block, target_block1) | target_block1.is_replaceable() {
        merge_slab(&mut block, target_block1);
        target1
    } else {
        let target2 = target1.adjacent(placement.face);
        let target_block2 = world.block_at(target2)?;
        if merge_slab(&mut block, target_block2) | waterlog(&mut block, target_block2) | target_block2.is_replaceable() {
            target2
        } else {
            return None;
        }
    };
    if chunk_entities.entities_in_chunk(target.chunk()).iter().any(|_entity| { false }) { // FIXME: Somehow check if block would collide with any entities
        return None;
    }
    if world.set_block_at(target, block) {
        Some(BlockChangeEvent::single(target))
    } else {
        None
    }
}

fn merge_slab(block: &mut BlockId, target: BlockId) -> bool {
    let opt = try_merge_slabs(*block, target);
    *block = opt.unwrap_or(*block);
    opt.is_some()
}

fn try_merge_slabs(a: BlockId, b: BlockId) -> Option<BlockId> {
    if a.kind() != b.kind() {
        return None;
    }
    match (a.slab_kind(), b.slab_kind()) {
        (Some(c), Some(d)) => match (c, d) {
            (SlabKind::Top, SlabKind::Bottom) | (SlabKind::Bottom, SlabKind::Top) => Some(a.with_slab_kind(SlabKind::Double)),
            _ => None
        },
        _ => None
    }
}

/// Determine what kind of slab to place. Returns `true` if the slab placed would be merged with the other slab. `try_merge_slabs` should always succeed if this function returns `true`.
fn slab_to_place(block: &mut BlockId, target: BlockId, placement: &BlockPlacementEvent) -> bool {
    let (slab_kind, place_adjacent) = match placement.cursor_position[1] {
        y if y == 0.5 => {
            if let Some(k) = target.slab_kind() {
                if block.kind() != target.kind() {
                    match k {
                        SlabKind::Top => (SlabKind::Top, false),
                        SlabKind::Bottom => (SlabKind::Bottom, false),
                        SlabKind::Double => return false
                    }
                } else {
                    match k {
                        SlabKind::Top => (SlabKind::Bottom, true),
                        SlabKind::Bottom => (SlabKind::Top, true),
                        SlabKind::Double => return false,
                    }
                }
            } else { return false; }
        }
        y if y == 0.0 => (SlabKind::Top, false),
        y if matches!(placement.face, BlockFace::Top) || y == 1.0 => (SlabKind::Bottom, false),
        y if y <  0.5 => (SlabKind::Bottom, false),
        y if y <  1.0 => (SlabKind::Top, false),
        _ => return false,
    };
    block.set_slab_kind(slab_kind);
    place_adjacent
}

fn waterlog(block: &mut BlockId, target: BlockId) -> bool {
    let mut waterloggable = if matches!(block.kind(), BlockKind::Water) { target } else if matches!(target.kind(), BlockKind::Water) { *block } else { return false };
    if waterloggable.slab_kind().map(|e| matches!(e, SlabKind::Double)).unwrap_or(false)
     | waterloggable.waterlogged().unwrap_or(false) {
        return false;
    }
    let succ = waterloggable.set_waterlogged(true);
    if succ { *block = waterloggable; }
    succ
}

fn set_face(block: &mut BlockId, placement: &BlockPlacementEvent) {
    if !matches!(placement.face, BlockFace::Top) {
        make_wall_block(block);
    }
    block.set_face(match placement.face {
        BlockFace::Bottom => Face::Ceiling,
        BlockFace::Top => Face::Floor,
        BlockFace::North => Face::Wall,
        BlockFace::South => Face::Wall,
        BlockFace::West => Face::Wall,
        BlockFace::East => Face::Wall,
    });
    block.set_facing_cardinal(match placement.face {
        BlockFace::Bottom => FacingCardinal::South,
        BlockFace::Top => FacingCardinal::South,
        BlockFace::North => FacingCardinal::North,
        BlockFace::South => FacingCardinal::South,
        BlockFace::West => FacingCardinal::West,
        BlockFace::East => FacingCardinal::East,
    });
    block.set_axis_xyz(match placement.face {
        BlockFace::Bottom => AxisXyz::Y,
        BlockFace::Top => AxisXyz::Y,
        BlockFace::North => AxisXyz::Z,
        BlockFace::South => AxisXyz::Z,
        BlockFace::West => AxisXyz::X,
        BlockFace::East => AxisXyz::X,
    });
    block.set_facing_cardinal_and_down(match placement.face {
        BlockFace::Bottom => FacingCardinalAndDown::Down,
        BlockFace::Top => FacingCardinalAndDown::Down,
        BlockFace::North => FacingCardinalAndDown::South,
        BlockFace::South => FacingCardinalAndDown::North,
        BlockFace::West => FacingCardinalAndDown::East,
        BlockFace::East => FacingCardinalAndDown::West,
    });
    set_stair_top_down(block, placement);
}

fn set_stair_top_down(block: &mut BlockId, placement: &BlockPlacementEvent) -> bool {
    block.set_half_top_bottom(match placement.face {
        BlockFace::Top => HalfTopBottom::Bottom,
        BlockFace::Bottom => HalfTopBottom::Top,
        _ => match placement.cursor_position[1] {
            y if y <= 0.5 => HalfTopBottom::Bottom,
            y if y >  0.5 => HalfTopBottom::Top,
            _ => return false,
        }
    })
}

fn make_wall_block(block: &mut BlockId) {
    *block = match block.kind() {
        BlockKind::Torch => BlockId::wall_torch(),
        BlockKind::RedstoneTorch => BlockId::redstone_wall_torch(),
        BlockKind::SoulTorch => BlockId::soul_wall_torch(),
        BlockKind::OakSign => BlockId::oak_wall_sign(),
        BlockKind::BirchSign => BlockId::birch_wall_sign(),
        BlockKind::AcaciaSign => BlockId::acacia_wall_sign(),
        BlockKind::JungleSign => BlockId::jungle_wall_sign(),
        BlockKind::SpruceSign => BlockId::spruce_wall_sign(),
        BlockKind::WarpedSign => BlockId::warped_wall_sign(),
        BlockKind::CrimsonSign => BlockId::crimson_wall_sign(),
        BlockKind::DarkOakSign => BlockId::dark_oak_wall_sign(),
        BlockKind::RedBanner => BlockId::red_wall_banner(),
        BlockKind::BlueBanner => BlockId::blue_wall_banner(),
        BlockKind::CyanBanner => BlockId::cyan_wall_banner(),
        BlockKind::GrayBanner => BlockId::gray_wall_banner(),
        BlockKind::LimeBanner => BlockId::lime_wall_banner(),
        BlockKind::PinkBanner => BlockId::pink_wall_banner(),
        BlockKind::BlackBanner => BlockId::black_wall_banner(),
        BlockKind::BrownBanner => BlockId::brown_wall_banner(),
        BlockKind::GreenBanner => BlockId::green_wall_banner(),
        BlockKind::WhiteBanner => BlockId::white_wall_banner(),
        BlockKind::OrangeBanner => BlockId::orange_wall_banner(),
        BlockKind::PurpleBanner => BlockId::purple_wall_banner(),
        BlockKind::YellowBanner => BlockId::yellow_wall_banner(),
        BlockKind::MagentaBanner => BlockId::magenta_wall_banner(),
        BlockKind::LightBlueBanner => BlockId::light_blue_wall_banner(),
        BlockKind::LightGrayBanner => BlockId::light_gray_wall_banner(),
        _ => *block,
    };
}

fn item_to_block(item: Item) -> Option<BlockId> {
    Some(match item {
        Item::WaterBucket => BlockId::water(),
        Item::LavaBucket => BlockId::lava(),
        Item::Redstone => BlockId::redstone_wire(),
        i => { 
            let mut name = "minecraft:".to_owned();
            name.push_str(i.name());
            return BlockId::from_identifier(&name)
        }
    })
}

fn decrease_slot(slot: &mut Option<ItemStack>) {
    match slot  {
        Some(s) => {
            match s.item() {
                Item::WaterBucket | Item::LavaBucket => s.set_item(Item::Bucket),
                _ => {
                    s.remove(1);
                    if s.count() == 0 {
                        *slot = None;
                    }
                }
            }
        },
        None => {},
    }
}