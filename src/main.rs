use bevy::{
    input::{common_conditions::input_just_released, mouse::AccumulatedMouseMotion},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused},
};
use rand::{SeedableRng, seq::IndexedRandom};

#[derive(Component)]
struct Player {
    sensitivity: f32,
    speed: f32,
}

#[derive(Event, Deref)]
struct GrabEvent(bool);

impl Default for Player {
    fn default() -> Player {
        Player {
            sensitivity: 100.,
            speed: 50.,
        }
    }
}

#[derive(Component, Message)]
struct BallSpawn {
    position: Vec3,
    velocity: Vec3,
}

#[derive(Resource)]
struct BallData {
    mesh: Handle<Mesh>,
    materials: Vec<Handle<StandardMaterial>>,
    rng: std::sync::Mutex<rand::rngs::StdRng>,
}

impl FromWorld for BallData {
    fn from_world(world: &mut World) -> Self {
        use rand::SeedableRng;
        let mesh = world.resource_mut::<Assets<Mesh>>().add(Sphere::new(1.));
        let mut materials = Vec::new();
        let mut mat_assets = world.resource_mut::<Assets<StandardMaterial>>();
        for i in 0..36 {
            let color = Color::hsl((i * 10) as f32, 1., 0.5);
            materials.push(mat_assets.add(StandardMaterial {
                base_color: color,
                ..Default::default()
            }));
        }
        let seed = *b"PhaestusFoxBevyBasicsRemastered0";
        BallData {
            mesh,
            materials,
            rng: std::sync::Mutex::new(rand::rngs::StdRng::from_seed(seed)),
        }
    }
}

impl BallData {
    fn mesh(&self) -> Handle<Mesh> {
        self.mesh.clone()
    }
    fn material(&self) -> Handle<StandardMaterial> {
        let mut rng = self.rng.lock().unwrap();
        self.materials.choose(&mut *rng).unwrap().clone()
    }
}

#[derive(Component, Default, Deref, DerefMut)]
struct Velocity(Vec3);

const GRAVITY: Vec3 = Vec3::new(0., -9.8, 0.);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .insert_resource(Time::<Fixed>::from_hz(30.))
        .add_systems(
            FixedUpdate,
            (
                apply_velocity,
                apply_gravity.before(apply_velocity),
                bounce.after(apply_velocity),
            ),
        )
        .add_systems(
            Update,
            (
                player_look,
                player_move.after(player_look),
                focus_event,
                toggle_grab.run_if(input_just_released(KeyCode::Escape)),
                spawn_ball,
                shoot_ball.before(spawn_ball).before(focus_event),
            ),
        )
        .add_observer(apply_grab)
        .add_message::<BallSpawn>()
        .init_resource::<BallData>()
        .run();
}

fn apply_grab(grab: On<GrabEvent>, mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor.grab_mode = if **grab {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !**grab;
}

fn apply_gravity(mut objects: Query<&mut Velocity>, time: Res<Time>) {
    let g = GRAVITY * time.delta_secs();
    for mut v in &mut objects {
        **v += g;
    }
}

fn bounce(mut balls: Query<(&Transform, &mut Velocity)>) {
    for (transform, mut velocity) in &mut balls {
        if transform.translation.y < 0. && velocity.y < 0. {
            velocity.y *= -1.;
        }
    }
}

fn focus_event(mut commands: Commands, mut events: MessageReader<WindowFocused>) {
    if let Some(event) = events.read().last() {
        commands.trigger(GrabEvent(event.focused));
    }
}

fn toggle_grab(mut commands: Commands, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.focused = !window.focused;
    commands.trigger(GrabEvent(window.focused));
}

fn spawn_camera(commands: &mut Commands) {
    commands.spawn((Camera3d::default(), Player::default()));
}

fn spawn_ball(
    mut commands: Commands,
    mut message: MessageReader<BallSpawn>,
    ball_data: Res<BallData>,
) {
    for spawn in message.read() {
        commands.spawn((
            Transform::from_translation(spawn.position),
            Mesh3d(ball_data.mesh()),
            MeshMaterial3d(ball_data.material()),
            Velocity(spawn.velocity),
        ));
    }
}

fn shoot_ball(
    input: Res<ButtonInput<MouseButton>>,
    player: Single<&Transform, With<Player>>,
    mut spawner: MessageWriter<BallSpawn>,
    cursor: Single<&mut CursorOptions>,
) {
    if cursor.visible {
        return;
    }
    if !input.just_pressed(MouseButton::Left) {
        return;
    }
    spawner.write(BallSpawn {
        position: player.translation,
        velocity: player.forward().as_vec3() * 15.,
    });
}

fn apply_velocity(mut objects: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut objects {
        transform.translation += velocity.0 * time.delta_secs();
    }
}

fn player_move(
    player: Single<(&mut Transform, &Player), With<Player>>,
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let mut delta = Vec3::ZERO;
    if input.pressed(KeyCode::KeyA) {
        delta.x -= 1.;
    }
    if input.pressed(KeyCode::KeyD) {
        delta.x += 1.;
    }
    if input.pressed(KeyCode::KeyS) {
        delta.z -= 1.;
    }
    if input.pressed(KeyCode::KeyW) {
        delta.z += 1.;
    }
    let (mut transform, player_data) = player.into_inner();
    let forward = transform.forward().as_vec3() * delta.z;
    let right = transform.right().as_vec3() * delta.x;
    let mut to_move = forward + right;
    to_move.y = 0.;
    to_move = to_move.normalize_or_zero();
    transform.translation += to_move * time.delta_secs() * player_data.speed;
}

fn player_look(
    player: Single<(&mut Transform, &Player), With<Player>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    if !window.focused {
        return;
    }
    let dt = time.delta_secs();
    use EulerRot::YXZ;
    let (mut transform, player_data) = player.into_inner();
    let sensitivity = player_data.sensitivity / window.width().min(window.height());
    let (mut yaw, mut pitch, _) = transform.rotation.to_euler(YXZ);
    pitch -= mouse_motion.delta.y * dt * sensitivity;
    yaw -= mouse_motion.delta.x * dt * sensitivity;
    pitch = pitch.clamp(-1.57, 1.57);
    transform.rotation = Quat::from_euler(YXZ, yaw, pitch, 0.);
}

fn spawn_map(mut commands: Commands, ball_data: Res<BallData>) {
    commands.spawn(DirectionalLight::default());
    for h in 0..ball_data.materials.len() {
        commands.spawn((
            Transform::from_translation(Vec3::new((-8. + h as f32) * 2., 0., -50.)),
            Mesh3d(ball_data.mesh()),
            MeshMaterial3d(ball_data.materials[h].clone()),
        ));
    }
}

fn setup(mut commands: Commands, ball_data: Res<BallData>) {
    spawn_camera(&mut commands);
    spawn_map(commands, ball_data);
}
