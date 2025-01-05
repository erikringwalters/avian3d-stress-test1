use std::time::Duration;

use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css,
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PresentMode,
    winit::{UpdateMode, WinitSettings},
};

// Amount of cubes to spawn (^3)
const CUBE_AXIS_AMOUNT:i32 = 10;

// Physics tick rate
const PHYSICS_HZ: f64 = 60.0;

// Environment
const FLOOR_RADIUS: f32 = 100.0;

// Player Controller
const MOVEMENT_SPEED: f32 = 10.;
const ROTATE_SPEED: f32 = 0.05;
const JUMP_SPEED: f32 = 75.0;
const GROUND_DISTANCE: f32 = 1.01;
const JUMP_COOLDOWN: f32 = 0.1;


#[derive(Component, Debug)]
pub struct Velocity {
    pub value: Vec3,
}

impl Velocity {
    pub fn new(value: Vec3) -> Self {
        Self { value }
    }
}

#[derive(Component, Debug)]
pub struct PlayerController {
    pub velocity: Velocity,
    pub jump_timer: Timer,
    pub is_on_ground: bool,
}

fn main() {
    App::new()
        .insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ))
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins((
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin,
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        ))
        .add_plugins(PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()))
        .add_systems(Startup, setup)
        .add_systems(
            FixedPreUpdate,
            (
                check_is_on_ground,
                movement_controls,
                update_linear_velocity,
                apply_impulses,
            )
                .chain(),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let capsule_radius = 0.5;
    let capsule_half_length = 0.5;
    let capsule_length = capsule_half_length * 2.0;

    let cube_half_size = 0.4;
    let cube_size = cube_half_size * 2.0;

    let starting_position_offset = 10.0;

    let debug_material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(uv_debug_texture())),
        ..default()
    });

    // Floor
    commands.spawn((
        RigidBody::Static,
        Collider::cylinder(FLOOR_RADIUS, 0.1),
        (
            Mesh3d(meshes.add(Cylinder::new(FLOOR_RADIUS, 0.1))),
            MeshMaterial3d(debug_material.clone()),
        ),
        Friction::new(1.0),
        Restitution::new(0.1),
    ));

    let light_distance = 1000.0;

    // Directional Light
    commands.spawn((
        DirectionalLight {
            illuminance: 2500.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::from_xyz(light_distance, light_distance, -light_distance).looking_at(-Vec3::Y, Vec3::Z),
    ));

    // let color_step = 1.0 / CUBE_AXIS_AMOUNT as f32;

    // Cubes
    for i in 0..CUBE_AXIS_AMOUNT {
        for j in 0..CUBE_AXIS_AMOUNT {
            for k in 0..CUBE_AXIS_AMOUNT {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid {
                        half_size: Vec3::new(cube_half_size, cube_half_size, cube_half_size),
                    })),
                    MeshMaterial3d(materials.add(Color::from(
                        css::SKY_BLUE

                    //     Srgba {
                    //     red: i as f32 * color_step,
                    //     green: j as f32 * color_step,
                    //     blue: k as f32 * color_step,
                    //     alpha: 1.0,
                    // }
                ))),
                    Transform::from_xyz(
                        i as f32 + cube_half_size - (CUBE_AXIS_AMOUNT as f32 / 2.0),
                        j as f32 + starting_position_offset,
                        k as f32 + starting_position_offset / 2.0,
                    ),
                    RigidBody::Dynamic,
                    Mass(10.0),
                    Friction::new(0.9),
                    Collider::cuboid(cube_size, cube_size, cube_size),
                ));
            }
        }
    }

    // Capsule
    let player = commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d {
                radius: capsule_radius,
                half_length: capsule_half_length,
            })),
            MeshMaterial3d(materials.add(Color::from(css::LIGHT_GREEN))),
            Transform::from_xyz(0.0, 2.0, 0.0),
            RigidBody::Dynamic,
            Collider::capsule(capsule_radius, capsule_length),
            Mass(10.),
            GravityScale(2.0),
            ExternalImpulse::default(),
            LockedAxes::ROTATION_LOCKED,
            PlayerController {
                velocity: Velocity::new(Vec3::ZERO),
                jump_timer: Timer::new(Duration::from_secs_f32(JUMP_COOLDOWN), TimerMode::Once),
                is_on_ground: false,
            },
        ))
        .id();

    // Spawn Player's Children

    // Camera
    let child_camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0., 2.0, -10.0).looking_at(Vec3{x: 0.0, y: 1.0, z: 0.0}, Dir3::Y),
        ))
        .id();

    // Pointer Cone
    let child_cone = commands
        .spawn((
            Mesh3d(meshes.add(Cone {
                radius: capsule_radius * 0.5,
                height: capsule_length * 2.0,
            })),
            Transform::from_xyz(0.0, capsule_half_length, 0.0).looking_at(Vec3::ZERO, Dir3::Z),
            MeshMaterial3d(materials.add(Color::from(css::LIGHT_PINK))),
        ))
        .id();

    //Raycaster and Query Filter
    let query_filter = SpatialQueryFilter::from_mask(0b1011).with_excluded_entities([player]);
    let child_raycaster = commands
        .spawn(RayCaster::new(Vec3::ZERO, -Dir3::Y).with_query_filter(query_filter))
        .id();

    //Add Children to Player
    commands
        .entity(player)
        .add_children(&[child_camera, child_cone, child_raycaster]);
}

fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn movement_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &mut PlayerController), With<PlayerController>>,
    time: Res<Time>,
) {
    let mut forward_movement = 0.0;
    let mut side_movement = 0.0;
    let mut upward_movement = 0.0;
    let mut h_vel: Vec3;

    let Ok((mut transform, mut player_controller)) = query.get_single_mut() else {
        println!("Could not query!");
        return;
    };

    player_controller
        .jump_timer
        .tick(Duration::from_secs_f32(time.delta_secs()));

    // TODO: Limit controls while airborne
    if keyboard.pressed(KeyCode::KeyW) {
        forward_movement = 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        forward_movement = -1.0;
    }
    if keyboard.pressed(KeyCode::KeyQ) {
        side_movement = 1.0;
    }
    if keyboard.pressed(KeyCode::KeyE) {
        side_movement = -1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        transform.rotate_y(ROTATE_SPEED);
    }
    if keyboard.pressed(KeyCode::KeyD) {
        transform.rotate_y(-ROTATE_SPEED);
    }
    if keyboard.pressed(KeyCode::Space) {
        if !(player_controller.jump_timer.remaining() > Duration::ZERO)
            && player_controller.is_on_ground
        {
            upward_movement = 1.0;
            player_controller.jump_timer.reset();
        }
    }

    // Normalize horizontal movement
    h_vel = (-transform.forward() * forward_movement) + (-transform.left() * side_movement);
    if !is_near_zero(forward_movement) || !is_near_zero(side_movement) {
        h_vel = MOVEMENT_SPEED * h_vel.normalize();
    }

    player_controller.velocity.value = h_vel;
    player_controller.velocity.value.y = upward_movement * JUMP_SPEED;
    // println!("{:?}", vel);
    // println!("{:?}", player_controller.jump_timer.remaining());
}

fn update_linear_velocity(
    mut query: Query<(&mut LinearVelocity, &mut PlayerController), With<PlayerController>>,
) {
    for (mut linear_velocity, player_controller) in query.iter_mut() {
        let vel = LinearVelocity(player_controller.velocity.value);
        linear_velocity.x = vel.x;
        linear_velocity.z = vel.z;
    }
}

fn apply_impulses(
    mut query: Query<(&mut ExternalImpulse, &PlayerController), With<PlayerController>>,
) {
    for (mut external_impulse, player_controller) in query.iter_mut() {
        let vel = player_controller.velocity.value.y;
        external_impulse.apply_impulse(Vec3::Y * vel);
    }
}

fn is_near_zero(value: f32) -> bool {
    value > -0.001 && value < 0.001
}

fn check_is_on_ground(
    mut player_query: Query<&mut PlayerController>,
    mut ray_query: Query<&RayHits>,
) {
    let Ok(mut player_controller) = player_query.get_single_mut() else {
        println!("Could not query!");
        return;
    };

    // In case ray hit doesn't find anything
    player_controller.is_on_ground = false;

    for hits in ray_query.iter_mut() {
        for hit in hits.iter_sorted() {
            // println!("Hit entity {} at distance {}", hit.entity, hit.distance,);

            // Only check first ray hit
            player_controller.is_on_ground = hit.distance <= GROUND_DISTANCE;
            // println!("{:?}", player_controller.is_on_ground);
            return;
        }
    }
}
