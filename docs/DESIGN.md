# GxEngine 设计文档

## 1. 项目概述

### 1.1 项目定位

GxEngine 是一个轻量级、模块化的 3D 游戏引擎，专注于 FPS 游戏开发。采用 Rust 语言编写，基于 ECS 架构，提供现代化的渲染管线和物理模拟能力。

### 1.2 设计目标

- **模块化架构**：各子系统独立，可按需组合
- **高性能**：充分利用 Rust 的零成本抽象和并发能力
- **易于扩展**：清晰的接口设计，方便添加新功能
- **学习友好**：代码结构清晰，适合理解引擎原理

### 1.3 技术栈

| 组件 | 技术选型 | 说明 |
|------|----------|------|
| 核心语言 | Rust 2021 (1.95+) | 内存安全、高性能 |
| 图形 API | wgpu 29.0 | 跨平台现代图形抽象层 |
| 物理引擎 | rapier3d 0.32 | Rust 原生物理引擎 |
| ECS 框架 | hecs 0.11 | 成熟轻量级 ECS 库，保障高性能与借用安全 |
| 资产格式 | glTF 2.0 / GLB | 统一规范的 3D 运行时资产交换标准 |
| 窗口管理 | winit 0.30 | 跨平台窗口与输入事件管理 |

---

## 2. 系统架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      GxEngine 架构                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Input     │  │   Audio     │  │   Script    │         │
│  │   System    │  │   System    │  │   System    │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│  ┌──────┴────────────────┴────────────────┴──────┐         │
│  │              ECS Core (World)                 │         │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐      │         │
│  │  │Entities │  │Components│  │ Systems │      │         │
│  │  └─────────┘  └─────────┘  └─────────┘      │         │
│  └───────────────────────┬───────────────────────┘         │
│                          │                                  │
│  ┌───────────────────────┴───────────────────────┐         │
│  │              Core Systems                     │         │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐      │         │
│  │  │Rendering│  │ Physics │  │  Kinemat│      │         │
│  │  │ System  │  │ System  │  │  System │      │         │
│  │  └─────────┘  └─────────┘  └─────────┘      │         │
│  └───────────────────────────────────────────────┘         │
│                          │                                  │
│  ┌───────────────────────┴───────────────────────┐         │
│  │              Backend                          │         │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐      │         │
│  │  │  wgpu   │  │ rapier3d│  │  winit  │      │         │
│  │  └─────────┘  └─────────┘  └─────────┘      │         │
│  └───────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 模块依赖关系

```
gxengine-core (ECS、数学库、基础类型)
      │
      ├── gxengine-renderer (wgpu 渲染后端)
      │
      ├── gxengine-physics (rapier3d 物理后端)
      │
      ├── gxengine-input (输入处理)
      │
      └── gxengine-asset (资产加载与管理)
```

---

## 3. 核心模块设计

### 3.1 ECS 核心 (gxengine-core)

#### 3.1.1 设计理念

采用成熟轻量级的 **`hecs`**（或其高度简化的安全仿制版）作为 ECS 核心容器，通过 Archetype-based 架构解决 Rust 的生命周期与借用检查难点，兼顾高性能与极致的借用安全：

- **Entity**：类型安全的轻量级 ID 标识（包含 Generation 以防止 ID 复用问题）
- **Component**：纯数据结构（SoA/Archetype 紧凑存储），最大限度提高 CPU 缓存命中率
- **System**：独立的逻辑函数/组件查询器，通过 `hecs` 的 `Query` 机制安全并发或单线程更新

#### 3.1.2 核心数据结构与接口

```rust
use hecs::{World, Entity, Query};

// 引擎核心容器包装
pub struct EngineWorld {
    pub ecs_world: World,
}

impl EngineWorld {
    pub fn new() -> Self {
        Self { ecs_world: World::new() }
    }

    // 生成新实体
    pub fn spawn(&mut self) -> Entity {
        self.ecs_world.spawn(())
    }

    // 添加组件
    pub fn add_component<C: hecs::Component>(&mut self, entity: Entity, component: C) {
        self.ecs_world.insert_one(entity, component).unwrap();
    }

    // 安全查询接口
    pub fn query<'a, Q: Query<'a>>(&'a self) -> hecs::QueryBorrow<'a, Q> {
        self.ecs_world.query::<Q>()
    }
}
```

#### 3.1.3 组件定义

```rust
// 基础组件
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

pub struct Health {
    pub current: f32,
    pub max: f32,
}

pub struct RenderMesh {
    pub mesh_handle: Handle<Mesh>,
    pub material_handle: Handle<Material>,
}

pub struct Collider {
    pub shape: ColliderShape,
    pub is_static: bool,
}
```

### 3.2 渲染系统 (gxengine-renderer)

#### 3.2.1 渲染管线架构

采用前向渲染管线，通过引入 **PBR+IBL** 以及 **后处理多级链路** 确保中高端视觉效果（画质不塑料，高反光金属不黑）：

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            前向 PBR+IBL 渲染管线                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. 场景遍历与剔除 → 收集可渲染对象，进行视锥体裁剪 (Frustum Culling)         │
│  2. 阴影图生成 → 绘制方向光 Shadow Map (级联阴影) 以及点/聚光灯全向阴影映射   │
│  3. 环境光预处理 → 离线或加载时计算 IBL 辐照度图 (Irradiance) 和辐射预滤波图  │
│  4. 屏幕遮蔽渲染 → 计算屏幕空间环境光遮蔽 (SSAO) 纹理，增加暗部实体缝隙层次   │
│  5. 主渲染 Pass → 执行 PBR 光照着色 (MSAA 4x 抗锯齿 + IBL 环境混合)           │
│  6. HDR 后处理链路 → 提取高光 (Bloom 提取) → 模糊混合 → ACES Tone Mapping      │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 3.2.2 渲染数据流与绑定组设计 (Bind Groups)

为了极致降低渲染状态切换开销，wgpu 资源按更新频次被划分为三级绑定组：

```rust
// 绑定组 0 (每帧一次)：全局全局空间数据及环境 IBL
pub struct GlobalBindGroup {
    pub view_proj: Mat4,
    pub camera_pos: Vec3,
    pub time: f32,
    pub ambient_light: Vec4,
    pub irradiance_cubemap: wgpu::TextureView,      // IBL 漫反射天空反射
    pub prefiltered_specular: wgpu::TextureView,    // IBL 镜面反射预滤波图
    pub brdf_lut: wgpu::TextureView,                // BRDF 查找纹理
}

// 绑定组 1 (每个材质一次)：PBR 参数与贴图
pub struct MaterialBindGroup {
    pub base_color_factor: Vec4,
    pub pbr_factors: Vec4, // x: metallic, y: roughness
    pub albedo_map: wgpu::TextureView,
    pub normal_map: wgpu::TextureView,
    pub metallic_roughness_map: wgpu::TextureView,
}

// 绑定组 2 (每个物体一次)：变换与骨骼动画
pub struct ObjectBindGroup {
    pub model_matrix: Mat4,
    pub joint_matrices: Vec<Mat4>, // 骨骼动画节点变换矩阵最大支持 256 骨骼
}

pub struct RenderPipeline {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub depth_texture: wgpu::Texture,
    pub msaa_texture: wgpu::Texture, 
    pub ssao_texture: wgpu::Texture, 
    pub shader_cache: HashMap<String, wgpu::ShaderModule>,
}
```

#### 3.2.3 材质系统

```rust
pub struct Material {
    pub name: String,
    pub albedo_factor: Color,
    pub metallic: f32,
    pub roughness: f32,
    pub albedo_map: Option<Handle<Texture>>,
    pub normal_map: Option<Handle<Texture>>,
    pub metallic_roughness_map: Option<Handle<Texture>>,
    pub occlusion_map: Option<Handle<Texture>>,
    pub shader: Handle<Shader>,
}
```

### 3.3 物理系统 (gxengine-physics)

#### 3.3.1 物理世界封装

```rust
pub struct PhysicsWorld {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub gravity: Vec3,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
}
```

#### 3.3.2 碰撞体类型

```rust
pub enum ColliderShape {
    Sphere { radius: f32 },
    Cuboid { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
    Mesh { vertices: Vec<Vec3>, indices: Vec<u32> },
}
```

### 3.4 动力学系统 (gxengine-kinematics)

#### 3.4.1 后坐力系统

基于二阶弹簧阻尼模型：

```
m * d²x/dt² + c * dx/dt + k * x = F(t)

其中：
- m: 质量（惯性）
- c: 阻尼系数（衰减速度）
- k: 刚度（回正速度）
- F(t): 脉冲力（开火时施加）
```

```rust
pub struct RecoilSystem {
    pub mass: f32,           // 惯性
    pub damping: f32,        // 阻尼
    pub stiffness: f32,      // 刚度
    pub current_offset: Vec2, // 当前偏移
    pub velocity: Vec2,      // 当前速度
    pub impulse_queue: Vec<Vec2>, // 待处理脉冲
}

impl RecoilSystem {
    pub fn apply_impulse(&mut self, impulse: Vec2) {
        self.impulse_queue.push(impulse);
    }

    pub fn update(&mut self, dt: f32) {
        // 处理脉冲队列
        for impulse in self.impulse_queue.drain(..) {
            self.velocity += impulse / self.mass;
        }

        // 弹簧阻尼积分
        let spring_force = -self.stiffness * self.current_offset;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        self.velocity += acceleration * dt;
        self.current_offset += self.velocity * dt;
    }
}
```

#### 3.4.2 IK 系统

FABRIK (Forward And Backward Reaching Inverse Kinematics) 实现：

```rust
pub struct IKSolver {
    pub max_iterations: u32,
    pub tolerance: f32,
}

impl IKSolver {
    pub fn solve(
        &self,
        joints: &mut [Vec3],
        target: Vec3,
        constraints: &[JointConstraint],
    ) {
        let base = joints[0];
        let chain_length = joints.len();

        for _ in 0..self.max_iterations {
            // Forward reaching
            joints[chain_length - 1] = target;
            for i in (1..chain_length).rev() {
                let direction = (joints[i] - joints[i - 1]).normalize();
                joints[i - 1] = joints[i] - direction * constraints[i].length;
            }

            // Backward reaching
            joints[0] = base;
            for i in 0..chain_length - 1 {
                let direction = (joints[i + 1] - joints[i]).normalize();
                joints[i + 1] = joints[i] + direction * constraints[i + 1].length;
            }

            // 检查收敛
            if (joints[chain_length - 1] - target).magnitude() < self.tolerance {
                break;
            }
        }
    }
}
```

#### 3.4.3 射线检测

```rust
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
}

pub struct RaycastHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub entity: Entity,
}

pub struct RaycastSystem {
    pub physics_world: PhysicsWorld,
}

impl RaycastSystem {
    pub fn cast_ray(&self, ray: Ray) -> Option<RaycastHit> {
        // 使用 rapier3d 的 raycast 功能
        // 返回最近的碰撞点
    }
}
```

### 3.5 输入系统 (gxengine-input)

```rust
pub struct InputState {
    pub mouse_delta: Vec2,
    pub mouse_position: Vec2,
    pub keys: HashMap<KeyCode, ButtonState>,
    pub mouse_buttons: HashMap<MouseButton, ButtonState>,
}

pub enum ButtonState {
    Pressed,
    Released,
    Held,
}

pub struct InputSystem {
    pub state: InputState,
    pub event_queue: Vec<InputEvent>,
}
```

### 3.6 资产系统 (gxengine-asset)

```rust
pub struct AssetManager {
    pub meshes: AssetStore<Mesh>,
    pub textures: AssetStore<Texture>,
    pub materials: AssetStore<Material>,
    pub shaders: AssetStore<Shader>,
}

pub struct AssetStore<T> {
    assets: HashMap<Handle<T>, T>,
    next_handle: u64,
}

pub struct Handle<T> {
    pub id: u64,
    pub _marker: PhantomData<T>,
}
```

---

## 4. 资源策略

### 4.1 美术资源方案

由于没有美术资源，采用以下策略：

#### 4.1.1 占位资源

- **几何体**：使用程序化生成的基本形状
  - 立方体、球体、圆柱体、平面
  - 可通过参数调整细分级别

- **材质**：纯色 + 简单纹理
  - 基础颜色：白、灰、黑
  - 棋盘格纹理（程序化生成）
  - 法线贴图：平法线（0,0,1）

- **光照**：简单的方向光 + 环境光
  - 默认方向光：(1, -1, 0.5)
  - 环境光强度：0.1

#### 4.1.2 测试场景

```rust
pub struct TestScene {
    pub ground: Entity,      // 大型平面
    pub walls: Vec<Entity>,  // 墙壁立方体
    pub targets: Vec<Entity>, // 射击目标
    pub player: Entity,      // 玩家实体
}

impl TestScene {
    pub fn create(world: &mut World) -> Self {
        // 创建地面
        let ground = world.spawn();
        world.add_component(ground, Transform {
            position: Vec3::new(0.0, -0.5, 0.0),
            scale: Vec3::new(100.0, 1.0, 100.0),
            ..Default::default()
        });
        world.add_component(ground, RenderMesh {
            mesh_handle: assets.create_cube(),
            material_handle: assets.create_material(Color::GRAY),
        });
        world.add_component(ground, Collider {
            shape: ColliderShape::Cuboid {
                half_extents: Vec3::new(50.0, 0.5, 50.0)
            },
            is_static: true,
        });

        // 创建墙壁
        let mut walls = Vec::new();
        for i in 0..5 {
            let wall = world.spawn();
            world.add_component(wall, Transform {
                position: Vec3::new(i as f32 * 3.0, 1.5, -5.0),
                scale: Vec3::new(2.0, 3.0, 0.2),
                ..Default::default()
            });
            // ... 其他组件
            walls.push(wall);
        }

        // 创建射击目标
        let mut targets = Vec::new();
        for i in 0..3 {
            let target = world.spawn();
            world.add_component(target, Transform {
                position: Vec3::new(i as f32 * 4.0 - 4.0, 1.0, -10.0),
                scale: Vec3::new(1.0, 1.0, 0.1),
                ..Default::default()
            });
            // ... 其他组件
            targets.push(target);
        }

        // 创建玩家
        let player = world.spawn();
        world.add_component(player, Transform {
            position: Vec3::new(0.0, 1.6, 0.0),
            ..Default::default()
        });
        world.add_component(player, Camera::default());
        world.add_component(player, PlayerController::default());

        TestScene { ground, walls, targets, player }
    }
}
```

#### 4.1.3 程序化资源生成

```rust
pub struct ProceduralGenerator;

impl ProceduralGenerator {
    pub fn create_cube() -> Mesh {
        // 生成立方体顶点和索引
    }

    pub fn create_sphere(segments: u32, rings: u32) -> Mesh {
        // 生成球体
    }

    pub fn create_plane(size: f32) -> Mesh {
        // 生成平面
    }

    pub fn create_checkerboard_texture(size: u32) -> Texture {
        // 生成棋盘格纹理
    }
}
```

### 4.2 外部资源资产加载与管线标准

为了最大化资产兼容性、减小引擎体积并提高运行时性能，GxEngine 遵循“**强统一运行时，外部高兼容转换**”的资产策略：

#### 4.2.1 运行时资产规范

- **3D 场景与模型标准**：**唯一支持 glTF 2.0 / GLB 格式**。
  - 引擎不再直接提供 FBX、OBJ、MAX、Blend 等繁复格式的引擎内原生解析。
  - 所有非 glTF 美术资产，必须在外部 DCC 工具（如 Blender, Maya）中一键导出为标准的 `.gltf`（纹理分离）或 `.glb`（资源打包）格式再加载入引擎。
  - 支持完整的 glTF node 树状层级、网格体（Indices & Vertex Buffers）、基本材质（基于 PBR MR 模型）以及骨骼关键帧动画。
- **纹理资产**：
  - 运行时直接读取：PNG, JPEG（基础通道）、HDR 辐射度图（天空盒与 IBL 预计算源）。
  - （规划升级）**KTX2 / Basis Universal** 纹理压缩标准：在引擎后期，离线阶段将常规纹理压缩为 KTX2 (BC7/ASTC) 块压缩纹理，直接在 GPU 中读取而无需运行时解压，显存占用降低 75%。
- **着色器资产**：
  - 原生 WGSL (.wgsl) 着色器。通过 wgpu 在运行时热重载编译或预缓存。

#### 4.2.2 资产预处理管线 (Asset Cooking) [离线工具集]

在生产环境下，采用一个简单的离线脚本或命令行工具：
1. 监视 `assets/raw/` 原始素材目录。
2. 当有 OBJ/FBX 变动时，调用后台 Blender 命令行或第三方轻量级转换组件（如 `gltf-pipeline` / `assimp-cli`），自动生成 `assets/models/*.glb` 并输出至运行时资产夹。
3. 对大尺寸高画质纹理执行 `basisu` 压缩，转为 `.ktx2` 并生成材质关联 JSON 描述文件。

---

## 5. 输入绑定

### 5.1 默认键位

| 操作 | 键位 |
|------|------|
| 移动 | WASD |
| 跳跃 | Space |
| 蹲下 | Left Ctrl |
| 射击 | 鼠标左键 |
| 瞄准 | 鼠标右键 |
| 切换武器 | 1-5 |
| 重载 | R |
| 交互 | E |
| 暂停 | Escape |

### 5.2 鼠标设置

```rust
pub struct MouseSettings {
    pub sensitivity: f32,      // 默认 0.1
    pub invert_y: bool,        // 默认 false
    pub raw_input: bool,       // 默认 true
}
```

---

## 6. 游戏循环

### 6.1 主循环流程

```
┌─────────────────────────────────────────────────────────┐
│                    主游戏循环                            │
├─────────────────────────────────────────────────────────┤
│  1. 处理窗口事件 (winit)                                │
│  2. 收集输入状态                                        │
│  3. 更新 ECS 系统：                                     │
│     - Input System                                      │
│     - Player Controller                                 │
│     - Physics System (固定时间步长)                     │
│     - Kinematics System (后坐力、IK)                    │
│     - Animation System                                  │
│  4. 渲染：                                              │
│     - 收集渲染数据                                      │
│     - 执行渲染管线                                      │
│     - 呈现画面                                          │
│  5. 计算帧时间，控制帧率                                │
└─────────────────────────────────────────────────────────┘
```

### 6.2 时间管理

```rust
pub struct Time {
    pub delta: Duration,      // 帧间隔
    pub elapsed: Duration,    // 游戏运行总时间
    pub fixed_timestep: f32,  // 物理固定时间步长 (1/60)
    pub time_scale: f32,      // 时间缩放
}

impl Time {
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32() * self.time_scale
    }
}
```

---

## 7. 项目结构

```
gx3d/
├── Cargo.toml
├── src/
│   ├── main.rs                 # 程序入口
│   ├── lib.rs                  # 库入口
│   ├── core/                   # 核心模块
│   │   ├── mod.rs
│   │   ├── ecs.rs              # ECS 实现
│   │   ├── math.rs             # 数学工具
│   │   ├── transform.rs        # 变换组件
│   │   └── time.rs             # 时间管理
│   ├── renderer/               # 渲染系统
│   │   ├── mod.rs
│   │   ├── pipeline.rs         # 渲染管线
│   │   ├── mesh.rs             # 网格数据
│   │   ├── material.rs         # 材质系统
│   │   ├── texture.rs          # 纹理管理
│   │   └── shader.rs           # 着色器管理
│   ├── physics/                # 物理系统
│   │   ├── mod.rs
│   │   ├── world.rs            # 物理世界
│   │   ├── collider.rs         # 碰撞体
│   │   └── raycast.rs          # 射线检测
│   ├── kinematics/             # 动力学系统
│   │   ├── mod.rs
│   │   ├── recoil.rs           # 后坐力系统
│   │   ├── ik.rs               # IK 系统
│   │   └── camera.rs           # 摄像机控制
│   ├── input/                  # 输入系统
│   │   ├── mod.rs
│   │   ├── keyboard.rs         # 键盘输入
│   │   └── mouse.rs            # 鼠标输入
│   ├── asset/                  # 资产系统
│   │   ├── mod.rs
│   │   ├── loader.rs           # 资产加载器
│   │   ├── mesh_loader.rs      # 网格加载
│   │   └── texture_loader.rs   # 纹理加载
│   └── game/                   # 游戏逻辑
│       ├── mod.rs
│       ├── player.rs           # 玩家控制器
│       └── weapon.rs           # 武器系统
├── assets/                     # 资源目录
│   ├── shaders/                # 着色器文件
│   │   ├── basic.wgsl
│   │   └── pbr.wgsl
│   ├── textures/               # 纹理文件
│   │   └── placeholder.png
│   └── models/                 # 模型文件
│       └── (待添加)
├── docs/                       # 文档目录
│   ├── DESIGN.md               # 设计文档
│   └── API.md                  # API 文档
└── tests/                      # 测试目录
    ├── ecs_test.rs
    ├── physics_test.rs
    └── renderer_test.rs
```

---

## 8. 开发阶段与增量式迭代路线图

为了保障项目能稳健运行，避免 Rust 编译器庞大的借用生命周期开销与 GPU 调试噩梦，**严格禁止一次性将全部系统编完**。引擎应遵循以下 **优先级（先做什么，后做什么）** 进行阶段式增量迭代开发：

```
第一阶段：窗口与最小渲染 MVP (底层骨架)
      │
      ▼
第二阶段：引入 hecs ECS 与组件数据更新 (驱动逻辑)
      │
      ▼
第三阶段：集成 rapier3d 物理与 FPS 玩家控制器 (交互骨架)
      │
      ▼
第四阶段：PBR+IBL 材质与全向阴影 (画质飞跃)
      │
      ▼
第五阶段：glTF 骨骼蒙皮动画与 IK 系统结合 (高级内容扩展)
```

### 🚀 第一阶段：窗口与最小渲染 MVP 搭建 (高优先级 - 最先进行)

**目标**：初始化依赖，配置 wgpu 交换链，在窗口中稳定渲染出一个单色/顶点色旋转立方体。
- **任务流程**：
  1. [ ] 初始化 `Cargo.toml` 项目，配置 `winit`, `wgpu`, `glam` 等最核心依赖。
  2. [ ] 封装 winit 窗口，搭建基础窗口事件循环（Event Loop）。
  3. [ ] 编写 wgpu 底层初始化（包括 Adapter、Device、Queue，以及 Surface、SwapChain 配置）。
  4. [ ] 编写简单的 WGSL 顶点/片元着色器，上传硬编码的立方体顶点数据与 Projection 变换矩阵。
  5. [ ] 实现基础帧循环（Frame Render Loop），执行 Clear Color 擦除并完成 Draw Call。
- **测试与验证**：`cargo run` 能在 Windows 窗口中以稳定 60 FPS+ 呈现一个旋转的 3D 立方体，缩放窗口时渲染画面无拉伸且能自动 resize。

### ⚙️ 第二阶段：集成 `hecs` ECS 系统 (中高优先级)

**目标**：引入高效率组件库，将实体（Entity）与组件（Component）的存取逻辑从渲染底层解耦，用数据更新驱动实体。
- **任务流程**：
  1. [ ] 引入 `hecs` 依赖，设计包装 `EngineWorld`。
  2. [ ] 提取 `Transform` 和 `RenderMesh` 组件，重构第一阶段的绘制循环，包装为 `RenderingSystem`。
  3. [ ] 编写时间管理器（Time），添加 `MovementSystem`，安全地通过 Query 机制查询并修改带有 `Velocity` 组件实体的 `Transform` 坐标。
  4. [ ] 编写基础程序化几何体生成器（ProceduralGenerator），生成用于测试的地面、墙壁和大量漂浮立方体。
- **测试与验证**：场景中生成 1,000 个带 `Transform` 和 `Velocity` 的立方体，渲染帧率不下降，CPU 与 GPU 能够流畅渲染和位移组件。

### 🎮 第三阶段：物理系统集成与第一人称控制器 (中优先级)

**目标**：引入 `rapier3d` 物理模拟，捕获键盘鼠标输入，实现第一人称视角 FPS 玩家漫游以及场景刚体碰撞。
- **任务流程**：
  1. [ ] 引入 `rapier3d`，建立 `PhysicsWorld` 核心系统，设置重力与固定步长（1/60s）。
  2. [ ] 编写物理同步调度系统（PhysicsSyncSystem）：在物理运行前将 Kinematic 刚体的 `Transform` 同步进 Rapier；在模拟后将 Dynamic 刚体的物理坐标同步回 `Transform`。
  3. [ ] 编写 `InputSystem`，精确捕获 winit 键盘 WASD / Space / Ctrl 状态与鼠标每帧 Delta 偏移量。
  4. [ ] 结合 Rapier 的 Character Controller 编写 `PlayerControllerSystem`，实现摄像机第一人称视角俯仰角转换与带重力的物理移动。
  5. [ ] 编写物理射线检测系统（RaycastSystem），实现射击准星与场景物体的瞬间交点检测。
- **测试与验证**：玩家可以通过 WASD 控制第一人称视角移动，遇到墙壁能阻挡，掉落边缘会受重力下落，对准地面箱子开枪能发射射线检测到实体 ID。

### 🎨 第四阶段：画质飞跃与环境物理渲染 (中低优先级)

**目标**：提升视觉表现，从“开发画质”迈向“3D 现代画质”，杜绝廉价塑料感与边缘死白。
- **任务流程**：
  1. [ ] 扩展 WGSL 着色器，引入标准 PBR (Cook-Torrance BRDF) 数学公式（包括 Normal map、Roughness、Metallic 解析）。
  2. [ ] **核心步骤**：引入天空盒材质与 **IBL（基于图像的光照）预处理**。提取环境 HDR 立方体贴图，烘焙生成 Irradiance Map 和 Prefiltered Map，确保金属与粗糙表面在阴影下能正确漫反射与镜面反射天空环境。
  3. [ ] 支持级联方向光阴影（Cascaded Shadow Maps）以遮蔽日光，同时引入点光源立方体深度贴图（Omnidirectional Shadow Maps）支持局部光源投射阴影。
  4. [ ] 引入后处理管线多级渲染链：主渲染绘制到 HDR RGBA16Float 帧缓冲区 → 经过 Bloom（发光拉伸）提取发光物体 → 叠加至 ACES 曲线进行 Tone Mapping 降噪防曝光，最终输出至 SwapChain 呈现。
  5. [ ] 实现多重采样抗锯齿（MSAA 4x）和简单的环境光遮蔽（SSAO），提升物体相接处的重量感。
- **测试与验证**：生成金属球、粗糙球、镜面球，观察其在太阳光和阴影中是否都有来自天空反射的高质感画面；进入室内点亮一个灯泡，光源周围产生 Bloom光晕，物体在灯光下能产生正确的点光源阴影。

### 🦕 第五阶段：外部资产解析、骨骼蒙皮动画与高级 Kinematics (低优先级 - 最后做)

**目标**：彻底打通高自由度美术资源。加载带骨骼、网格和动画的 `.glb` 文件，并融入高级动力学系统。
- **任务流程**：
  1. [ ] 编写 `glTF` 资源解析器（通过 `gltf` 库），读取 `.glb` 文件的完整节点树层级关系、PBR 材质集及动画关键帧数据。
  2. [ ] 在顶点着色器中实现 Vertex Skinning 顶点蒙皮着色器，支持 4 骨骼权重混合渲染。
  3. [ ] 编写 `AnimationSystem`，根据时间插值骨骼节点的 Local 旋转和位移，动态合成当前帧骨骼变换矩阵数组（上传至 Bind Group 2 传递给 GPU）。
  4. [ ] 结合骨骼树层级，完善 **FABRIK IK 求解器**。将解算出的世界空间坐标映射回骨骼局部四元数旋转，实现脚部贴合复杂地形（IK Foot Placement）与手部贴紧枪支组件。
  5. [ ] 挂载二阶弹簧阻尼后坐力（RecoilSystem）至玩家 Camera 节点，完成开火瞬间脉冲插值摄像机抖动。
- **测试与验证**：在外部下载一个带开火/跑步动画的 glb 人物模型，引擎能流畅渲染动画；玩家开火时，相机有平滑反弹抖动的后坐力效果；当人物走上斜坡，其双脚高度能通过 IK 自动弯曲膝盖并稳稳踩在斜坡面上。

---

## 9. 依赖配置

```toml
[package]
name = "gxengine"
version = "0.1.0"
edition = "2021"

[dependencies]
# 窗口和事件
winit = "0.30"
# 图形 API
wgpu = "29.0"
# 数学库
glam = "0.33"
# 物理引擎
rapier3d = "0.32"
# 图像加载与基础转换
image = "0.25"
# glTF 加载
gltf = "1.4"
# 高效轻量级 ECS 容器
hecs = "0.11"
# 音频系统
kira = "0.12"
# 日志与辅助调试
log = "0.4"
env_logger = "0.11"
# 错误处理与抽象
anyhow = "1.0"
thiserror = "2.0"
# 并发 (用于特定并行计算如烘焙)
rayon = "1.10"
# 序列化与配置
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[dev-dependencies]
criterion = "0.5"
```

---

## 10. 设计决策记录

### 10.1 为什么选择成熟的 `hecs` 轻量级库而非完全自研 ECS？

**原因**：
1. **高性能与借用安全**：Rust 极其严格的借用检查使得在多线程和复杂 System 调度中，自研稀疏集容器极易陷入所有权分配陷阱。`hecs` 采用紧凑高效的 Archetype (原型) 内存结构设计，提供线程安全且零开销 of 借用方案。
2. **专注于核心能力开发**：不重复造轮子，使开发焦点从底层内存安全性问题，移向高画质 PBR 渲染、全向阴影、音频与第一人称 FPS 精密动力学控制上。
3. **极佳的解耦性**：`hecs` 没有任何引擎框架性绑定，能以最轻量的状态放入我们的 `gxengine-core` 组件。

### 10.2 为什么选择 rapier3d 而非 Jolt？

**原因**：
1. Rust 原生：无需 C++ 绑定
2. 社区活跃：文档和示例丰富
3. 性能足够：满足项目需求

### 10.3 为什么采用前向渲染而非延迟渲染？

**原因**：
1. 实现简单：适合原型开发
2. 兼容性好：支持透明物体
3. 性能足够：场景复杂度不高

**未来改进**：
- 可切换到延迟渲染
- 添加更多光照类型
- 实现全局光照

---

## 11. 附录

### 11.1 参考资源

- [wgpu 官方文档](https://docs.rs/wgpu)
- [rapier3d 官方文档](https://rapier.rs/docs/)
- [glTF 2.0 规范](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)
- [ECS 设计模式](https://github.com/SanderMertens/ecs-faq)

### 11.2 术语表

| 术语 | 说明 |
|------|------|
| ECS | Entity-Component-System，实体-组件-系统架构 |
| PBR | Physically Based Rendering，基于物理的渲染 |
| IK | Inverse Kinematics，逆向运动学 |
| G-Buffer | Geometry Buffer，几何缓冲区 |
| WGSL | WebGPU Shading Language，WebGPU 着色器语言 |
| SoA | Structure of Arrays，数组结构 |
| AoS | Array of Structures，结构数组 |

---

*文档版本：1.0*  
*最后更新：2026-05-26*  
*作者：GxEngine Team*
