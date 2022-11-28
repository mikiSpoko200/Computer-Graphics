extern crate alloc;

mod vertex;
mod geometry;
mod program;
mod uniform;
mod camera;
mod index_buffer;
mod consts;
mod drawing;

use std::default::Default;
use nalgebra_glm as glm;

use uniform::Uniform;
use program::Program;
use drawing::DrawMode;
use index_buffer::{IndexBuffer, IndexingMode, IndexType, IndexBufferObject};
use vertex::{VertexAttribute, BufferObject};

use glutin;
use gl;
use log;

use glutin::event::{Event, VirtualKeyCode, WindowEvent};
use glutin::event_loop::{EventLoop, ControlFlow};
use glutin::window::WindowBuilder;
use glutin::{Api, GlRequest};

// todo: Objects can emit painters which borrow data from them during upload.
//  data must be interpretable as &[VertexAttribute], &[IndexingPrimitive] and perhaps uniforms and programs.
//  assert that all buffer objects contain the same number of atts? what about indexing.
//  move instance_count information to buffer
const GL_VERSION: (u8, u8) = (4, 5);

#[macro_export]
macro_rules! gl_assert_no_err {
    () => {
        assert!(unsafe { gl::GetError() } == gl::NO_ERROR);
    }
}

#[macro_export]
macro_rules! gl_assert {
    ($s:stmt) => {
        $s
        if cfg!(debug_assertions) {
            let err = gl::GetError();
            match err {
                gl::NO_ERROR => {
                },
               _ => {
                    match err {
                        gl::INVALID_ENUM => panic!("GL_INVALID_ENUM"),
                        gl::INVALID_VALUE => panic!("GL_INVALID_VALUE"),
                        gl::INVALID_OPERATION => panic!("GL_INVALID_OPERATION"),
                        gl::INVALID_FRAMEBUFFER_OPERATION => panic!("GL_INVALID_FRAMEBUFFER_OPERATION"),
                        gl::OUT_OF_MEMORY => panic!("GL_OUT_OF_MEMORY"),
                        gl::STACK_UNDERFLOW => panic!("GL_STACK_UNDERFLOW"),
                        gl::STACK_OVERFLOW => panic!("GL_STACK_OVERFLOW"),
                        _ => panic!("unknown error")
                    }
                }
            }
        };
    }
}

pub struct Painter<I: IndexBuffer> {
    binder: Binder<I>,
    draw_mode: DrawMode,
    instance_count: Option<usize>,
}

impl<I: IndexBuffer> Painter<I> {
    pub fn new(binder: Binder<I>, draw_mode: DrawMode) -> Self {
        Self {
            binder,
            draw_mode,
            instance_count: None
        }
    }

    pub fn binder(&self) -> &Binder<I> {
        &self.binder
    }

    pub fn instanced(mut self, instance_count: usize) -> Self {
        self.instance_count = Some(instance_count);
        self
    }

    pub fn update_draw_mode(&mut self, new: DrawMode) {
        self.draw_mode = new;
    }

    pub fn draw(&self) {
        let _draw_scoped_binder = self.binder.draw_binder();
        match (self.instance_count, self.binder.index_type()) {
            (Some(instance_count), Some(ref index_type)) => {
                drawing::instanced::draw_indexed(
                    &self.draw_mode,
                    self.binder.vertex_count(),
                    index_type,
                    instance_count
                );
            },
            (Some(instance_count), None) => {
                drawing::instanced::draw_arrays(
                    &self.draw_mode,
                    self.binder.vertex_count(),
                    instance_count
                );
            },
            (None, Some(ref index_type)) => {
                drawing::draw_indexed(
                    &self.draw_mode,
                    self.binder.vertex_count(),
                    index_type
                );
            },
            (None, None) => {
                drawing::draw_arrays(&self.draw_mode, self.binder.vertex_count());
            }
        }
    }
}

// fixme: attribute / uniform layout provider - as of now layouts are specified in order.
//      quick solution -> print the manifest of (current layout - glsl lifetime - name)?

pub struct Binder<I: IndexBuffer> {
    vao: vertex::ArrayObject,
    vbos: Vec<Box<dyn vertex::Buffer>>,
    ebo: IndexingMode<I>,
    program: Program,
    uniforms: Vec<Box<dyn Uniform>>,
}

impl<I: IndexBuffer> Binder<I> {
    pub fn new(
        vbos: Vec<Box<dyn vertex::Buffer>>,
        ebo: IndexingMode<I>,
        program: Program,
        uniforms: Vec<Box<dyn Uniform>>,
    ) -> Self {
        let vao = vertex::ArrayObject::create();
        Self { vao, vbos, ebo, program, uniforms, }
    }

    pub fn upload(&mut self) {
        let _program_scoped_binder = self.program.scoped_binder();
        for (index, uniform) in self.uniforms.iter().enumerate() {
            uniform.bind(index as _);
        }

        let _vao_binder = self.vao.scoped_binder();
        for (index, vbo) in self.vbos.iter().enumerate() {
            let _scoped_binder = vbo.scoped_binder();
            gl_assert_no_err!();
            vbo.upload();
            gl_assert_no_err!();
            self.vao.set_vertex_attrib_pointer(index as _, &vbo.attribute_type());
            gl_assert_no_err!();
        }

        if let Some(ref index_buffer) = self.ebo {
            let _ebo_binder = index_buffer.scoped_binder();
            index_buffer.upload();
        }
    }

    pub fn vertex_count(&self) -> usize {
        if let Some(ref index_buffer) = self.ebo {
            index_buffer.vertex_count()
        } else {
            self.vbos.first().unwrap().vertex_count()
        }
    }

    pub(self) fn vao_binder(&self) -> vertex::array_object::ScopedBinder {
        self.vao.scoped_binder()
    }

    pub fn index_type(&self) -> Option<IndexType> {
        self.ebo.as_ref().map(|index_buffer| index_buffer.index_type())
    }

    pub(self) fn program_binder(&self) -> program::ScopedBinder { self.program.scoped_binder() }

    pub fn draw_binder(&self) -> DrawScopedBinder {
        DrawScopedBinder::new(self.program_binder(), self.vao_binder())
    }
}

pub struct DrawScopedBinder(program::ScopedBinder, vertex::array_object::ScopedBinder);

impl DrawScopedBinder {
    pub fn new(program: program::ScopedBinder, vao: vertex::array_object::ScopedBinder) -> Self {
        Self(program, vao)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CoordinateSystem {
    center: glm::Vec3,
    x: glm::Vec4,
    y: glm::Vec4,
    z: glm::Vec4,
}

impl CoordinateSystem {
    const CENTER: glm::Vec3 = glm::Vec3::new(0f32, 0f32, 0f32);
}

#[derive(Debug, Copy, Clone)]
pub struct Scene {
    bg_color: glm::Vec4,
    // skybox: geometry::Cube
}

impl Scene {
    const DARK_GRAY:  glm::Vec3 = glm::Vec3::new(0.23, 0.23, 0.23);
    const LIGHT_BLUE: glm::Vec3 = glm::Vec3::new(0.54, 0.82, 1.0);
}

#[derive(Debug, Copy, Clone)]
pub struct Directions {
    pub up: glm::Vec3,
    pub down: glm::Vec3,
    pub front: glm::Vec3,
    pub back: glm::Vec3,
    pub left: glm::Vec3,
    pub right: glm::Vec3,
}

impl Directions {
    const FRONT: glm::Vec3 = glm::Vec3::new( 0f32,  0f32,  1f32);
    const BACK:  glm::Vec3 = glm::Vec3::new( 0f32,  0f32, -1f32);
    const UP:    glm::Vec3 = glm::Vec3::new( 0f32,  1f32,  0f32);
    const DOWN:  glm::Vec3 = glm::Vec3::new( 0f32, -1f32,  0f32);
    const RIGHT: glm::Vec3 = glm::Vec3::new( 1f32,  0f32,  0f32);
    const LEFT:  glm::Vec3 = glm::Vec3::new(-1f32,  0f32,  0f32);
}

struct Triangle {
    binder: Binder<IndexBufferObject<u8>>
}

impl Triangle {
    pub fn new() -> Self {
        let triangle= attributes!(
            (-0.5, 0.0),
            ( 0.5, 0.0),
            ( 0.0, 0.8)
        );

        let colors = attributes!(
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0f32)
        );

        let scales = attributes!(
            0.5,
            0.5,
            0.5f32
        );

        let offsets = attributes!(
            (0.0, 0.5),
            (-0.5, -0.2),
            (0.5, -0.2f32),
        );

        let indices = vec!(1, 2, 3u8).into_boxed_slice();
        let index_buf = IndexBufferObject::create(indices);

        let program = Program::from_file(
            "shaders/triangle_v.glsl".as_ref(),
            "shaders/triangle_f.glsl".as_ref()
        );
        let mut binder = Binder::new(
            vec!(
                Box::new(triangle),
                Box::new(colors),
                Box::new(scales),
                Box::new(offsets),
            ),
            Some(index_buf),
            program,
            Vec::new()
        );

        binder.upload();
        {
            let _vao_binder = binder.vao_binder();
            binder.vao.set_attrib_divisor(2, 1);
            binder.vao.set_attrib_divisor(3, 1);
        }

        Self { binder }
    }
}

pub fn sp(radius: f32, poly_count: usize) -> (Box<[VertexAttribute<f32, 3>]>, Box<[VertexAttribute<f32, 3>]>, Box<[u16]>) {
    use std::f32::consts::PI;

    let mut vertices = Vec::new();
    let mut normals = Vec::new();

    let sector_angle_offset = 2.0 * PI / poly_count as f32;
    let stack_angle_offset = PI / poly_count as f32;

    for stack_index in 0..=poly_count {
        let stack_angle = PI / 2.0 - stack_index as f32 * stack_angle_offset;
        let xy = radius * f32::cos(stack_angle);
        let  z = radius * f32::sin(stack_angle);

        for sector_index in 0..=poly_count {
            let sector_angle = sector_index as f32 * sector_angle_offset;
            let x = xy * f32::cos(sector_angle);
            let y = xy * f32::sin(sector_angle);
            let point = glm::Vec3::new(x, y, z);

            vertices.push(VertexAttribute::from(*point.as_ref()));
            normals.push(VertexAttribute::from(*(point / radius).as_ref()));
        }
    }

    let mut indices = Vec::new();
    for stack_index in 0..poly_count {
        let k1 = (stack_index * (poly_count + 1)) as u16;
        let k2 = (k1 as usize + poly_count + 1) as u16;

        for (k1, k2) in (k1..).zip(k2..).take(poly_count) {
            if stack_index != 0 {
                indices.push(k1);
                indices.push(k2);
                indices.push(k1 + 1);
            }
            if stack_index != poly_count - 1 {
                indices.push(k1 + 1);
                indices.push(k2);
                indices.push(k2 + 1);
            }
        }
    }

    (vertices.into_boxed_slice(), normals.into_boxed_slice(), indices.into_boxed_slice())
}

pub fn sphere() -> Binder<IndexBufferObject<u16>> {
    let (vertices, normals, indices) = sp(1.0, 25);

    let positions = Box::new(BufferObject::create(vertices));
    let normals = Box::new(BufferObject::create(normals));
    let index_buf = IndexBufferObject::create(indices);

    let program = Program::from_file(
        "shaders/sphere_v.glsl".as_ref(),
        "shaders/sphere_f.glsl".as_ref()
    );

    let mut binder = Binder::new(
        vec!(positions, normals),
        Some(index_buf),
        program,
        vec!()
    );
    binder.upload();
    binder
}

pub fn template_triangle(a: f32) -> [glm::Vec3; 3] {
    let radius = a / f32::sqrt(3.0);
    [
        glm::vec3(0.0, radius, 0.0), // top point
        glm::vec3(-a / 2.0, -radius / 2.0, 0.0),
        glm::vec3(a / 2.0, -radius / 2.0, 0.0)
    ]
}

pub fn labyrinth(n: usize) -> Binder<IndexBufferObject> {
    let scale = 1.0 / n as f32;
    let tail_center_offset = glm::vec3(1f32, 1f32, 1f32) / (2.0 * n as f32);

    let scaled_model = template_triangle(2.0).into_iter().map(|position| {
        position * scale
    });
    let mut positions: Vec<VertexAttribute<f32, 3>> = Vec::new();

    for xi in 0..n {
        for yi in 0..n {
            for zi in 0..n {
                let corner_offset = glm::vec3(xi as _, yi as _, zi as _) / n as f32;
                let center_offset = corner_offset + tail_center_offset;
                let center_offset_ndc = 2.0 * center_offset - glm::vec3(1.0, 1.0, 1.0);
                for position in scaled_model.clone() {
                    let arr = (position + center_offset_ndc).as_ref().clone();
                    positions.push(VertexAttribute::from(arr));
                }
            }
        }
    }

    let buffer_object = BufferObject::create(positions.into_boxed_slice());

    let program = Program::from_file(
        "shaders/labyrinth_v.glsl".as_ref(),
        "shaders/labyrinth_f.glsl".as_ref()
    );

    let mut binder = Binder::new(vec!(Box::new(buffer_object)), None, program, vec!());
    binder.upload();
    binder
}

// todo: update framerate in the terminal in place.
fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_title("Learn OpenGL with Rust");

    // let aspect_ratio = window.

    let gl_context = glutin::ContextBuilder::new()
        .with_gl(GlRequest::Specific(Api::OpenGl, GL_VERSION))
        .build_windowed(window, &event_loop)
        .expect("Cannot create windowed context");

    let gl_context = unsafe {
        gl_context
            .make_current()
            .expect("Failed to make context current")
    };

    let size = gl_context.window().inner_size();
    let aspect_ratio = size.width as f32 / size.height as f32;
    let perspective = glm::perspective(aspect_ratio, f32::to_radians(120f32), 0.1, 100.0);
    let mut camera = glm::look_at(
        &glm::vec3(0f32, 0f32, 1f32),
        &CoordinateSystem::CENTER,
        &Directions::UP
    );

    let trans_right = glm::translation(&(0.01 * Directions::RIGHT));
    let trans_up    = glm::translation(&(0.01 * Directions::UP));
    let trans_front = glm::translation(&(0.01 * Directions::FRONT));
    let trans_left  = glm::translation(&(0.01 * Directions::LEFT));
    let trans_down  = glm::translation(&(0.01 * Directions::DOWN));
    let trans_back  = glm::translation(&(0.01 * Directions::BACK));

    let right_y_rotation_matrix = glm::rotation(f32::to_radians(0.1), &Directions::UP);
    let left_y_rotation_matrix = glm::rotation(-f32::to_radians(0.1), &Directions::UP);

    // gl_context.window().set_inner_size(glutin::dpi::LogicalSize::new(400.0, 200.0));
    // gl_context.window().set_fullscreen(Some(glutin::window::Fullscreen::Borderless(None)));
    // let size = gl_context.window().inner_size();
    // gl_context.(size);

    gl::load_with(|ptr| gl_context.get_proc_address(ptr) as *const _);

    let triangle = Triangle::new();
    let ball_painter = Painter::new(sphere(), DrawMode::Triangles);
    let triangle_painter = Painter::new(triangle.binder, DrawMode::Triangles).instanced(1000);
    let light_direction = Directions::DOWN + Directions::RIGHT + Directions::FRONT;

    let grid_size = 5;
    let labyrinth_painter = Painter::new(labyrinth(grid_size), DrawMode::Triangles).instanced(grid_size * grid_size * grid_size);

    {
        let _uniform_binder = triangle_painter.binder().program_binder();
        unsafe {
            gl_assert_no_err!();
            gl::UniformMatrix4fv(0, 1, gl::FALSE, perspective.as_ptr());
            gl_assert_no_err!();
            gl::UniformMatrix4fv(1, 1, gl::FALSE, camera.as_ptr());
            gl_assert_no_err!();
            gl::Uniform1f(2, 10.0);
            gl_assert_no_err!();
        }
    }

    {
        let _uniform_binder = ball_painter.binder.program_binder();
        unsafe {
            gl_assert_no_err!();
            gl::UniformMatrix4fv(0, 1, gl::FALSE, perspective.as_ptr());
            gl_assert_no_err!();
            gl::UniformMatrix4fv(1, 1, gl::FALSE, camera.as_ptr());
            gl_assert_no_err!();
            gl::Uniform3f(2, light_direction.x, light_direction.y, light_direction.z);
            gl_assert_no_err!();
        }
    }

    {
        let _uniform_binder = labyrinth_painter.binder.program_binder();
        unsafe {
            gl_assert_no_err!();
            gl::UniformMatrix4fv(0, 1, gl::FALSE, perspective.as_ptr());
            gl_assert_no_err!();
            gl::UniformMatrix4fv(1, 1, gl::FALSE, camera.as_ptr());
            gl_assert_no_err!();
        }
    }

    gl_assert_no_err!();
    unsafe { gl::Enable(gl::DEPTH_TEST); }
    gl_assert_no_err!();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::LoopDestroyed => (),
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(keycode) = input.virtual_keycode {
                        log::debug!("Updating position {:?}", keycode);
                        match keycode {
                            VirtualKeyCode::A => camera *= trans_left,
                            VirtualKeyCode::D => camera *= trans_right,
                            VirtualKeyCode::Q => camera *= trans_up,
                            VirtualKeyCode::Z => camera *= trans_down,
                            VirtualKeyCode::W => camera *= trans_front,
                            VirtualKeyCode::S => camera *= trans_back,
                            VirtualKeyCode::R => camera *= right_y_rotation_matrix,
                            VirtualKeyCode::L => camera *= left_y_rotation_matrix,
                            _ => (),
                        };
                        unsafe {
                            let _uniform_binder = triangle_painter.binder().program_binder();
                            gl_assert_no_err!();
                            gl::UniformMatrix4fv(1, 1, gl::FALSE, camera.as_ptr());
                            gl_assert_no_err!();
                        }
                        unsafe {
                            let _uniform_binder = ball_painter.binder().program_binder();
                            gl_assert_no_err!();
                            gl::UniformMatrix4fv(1, 1, gl::FALSE, camera.as_ptr());
                            gl_assert_no_err!();
                        }
                        unsafe {
                            let _uniform_binder = labyrinth_painter.binder().program_binder();
                            gl_assert_no_err!();
                            gl::UniformMatrix4fv(1, 1, gl::FALSE, camera.as_ptr());
                            gl_assert_no_err!();
                        }
                    }
                },
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::RedrawRequested(_) => {
                unsafe {
                    gl::ClearColor(Scene::LIGHT_BLUE.x, Scene::LIGHT_BLUE.y, Scene::LIGHT_BLUE.z, 1.0);
                    gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
                }
                gl_context.swap_buffers().unwrap();
            }
            _ => (),
        }

        unsafe { gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT); }
        // triangle_painter.draw();
        labyrinth_painter.draw();
        ball_painter.draw();
        gl_context.swap_buffers().unwrap();
    });
}
