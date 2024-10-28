#[macro_use]
extern crate glium;

use glium::{Display, Surface};
use glutin::surface::WindowSurface;
use support::{ApplicationContext, State};

mod support;

#[derive(Copy, Clone, Debug)]
struct PerInstance {
    pub id: u32,
    pub w_position: (f32, f32, f32),
    pub color: (f32, f32, f32),
}
implement_vertex!(PerInstance, id, w_position, color);

struct Application {
    pub vertex_buffer: glium::vertex::VertexBufferAny,
    pub program: glium::Program,
    pub picking_program: glium::Program,
    pub camera: support::camera::CameraState,
    pub per_instance: Vec<PerInstance>,
    pub original: Vec<PerInstance>,
    pub picking_attachments: Option<(glium::texture::UnsignedTexture2d, glium::framebuffer::DepthRenderBuffer)>,
    pub picking_pbo: glium::texture::pixel_buffer::PixelBuffer<u32>,
    pub cursor_position: Option<(i32, i32)>,
}

impl ApplicationContext for Application {
    const WINDOW_TITLE:&'static str = "Glium picking example";

    fn new(display: &Display<WindowSurface>) -> Self {
        // building the vertex and index buffers
        let vertex_buffer = support::load_wavefront(display, include_bytes!("support/teapot.obj"));

        // the program
        let program = program!(display,
            140 => {
                vertex: "
                    #version 140

                    uniform mat4 persp_matrix;
                    uniform mat4 view_matrix;

                    in uint id;
                    in vec3 w_position;
                    in vec3 color;
                    in vec3 position;
                    in vec3 normal;
                    out vec3 v_normal;
                    out vec3 v_color;

                    void main() {
                        v_normal = normal;
                        v_color = color;
                        gl_Position = persp_matrix * view_matrix * vec4(position * 0.005 + w_position, 1.0);
                    }
                ",

                fragment: "
                    #version 140

                    in vec3 v_normal;
                    in vec3 v_color;
                    out vec4 f_color;

                    const vec3 LIGHT = vec3(-0.2, 0.8, 0.1);

                    void main() {
                        float lum = max(dot(normalize(v_normal), normalize(LIGHT)), 0.0);
                        vec3 color = (0.3 + 0.7 * lum) * v_color;
                        f_color = vec4(color, 1.0);
                    }
                ",
            },
        ).unwrap();

        // the picking program
        let picking_program = program!(display,
            140 => {
                vertex: "
                    #version 140

                    uniform mat4 persp_matrix;
                    uniform mat4 view_matrix;

                    in uint id;
                    in vec3 w_position;
                    in vec3 color;
                    in vec3 position;
                    in vec3 normal;
                    flat out uint v_id;

                    void main() {
                        v_id = id;
                        gl_Position = persp_matrix * view_matrix * vec4(position * 0.005 + w_position, 1.0);
                    }
                ",

                fragment: "
                    #version 140

                    flat in uint v_id;
                    out uint f_id;

                    void main() {
                        f_id = v_id;
                    }
                ",
            },
        ).unwrap();

        let mut camera = support::camera::CameraState::new();
        camera.set_position((0.0, 0.0, -1.5));
        camera.set_direction((0.0, 0.0, 1.0));

        //id's must be unique and != 0
        let mut per_instance = vec![
            PerInstance { id: 1, w_position: (-1.0, 0.0, 0.0), color: (1.0, 0.0, 0.0)},
            PerInstance { id: 2, w_position: ( 0.0, 0.0, 0.0), color: (1.0, 0.0, 0.0)},
            PerInstance { id: 3, w_position: ( 1.0, 0.0, 0.0), color: (1.0, 0.0, 0.0)},
        ];
        per_instance.sort_by(|a, b| a.id.cmp(&b.id));
        let original = per_instance.clone();

        let picking_attachments: Option<(glium::texture::UnsignedTexture2d, glium::framebuffer::DepthRenderBuffer)> = None;
        let picking_pbo: glium::texture::pixel_buffer::PixelBuffer<u32>
            = glium::texture::pixel_buffer::PixelBuffer::new_empty(display, 1);


        let cursor_position: Option<(i32, i32)> = None;

        Self {
            vertex_buffer,
            program,
            picking_program,
            camera,
            per_instance,
            picking_attachments,
            picking_pbo,
            cursor_position,
            original,
        }
    }

    fn draw_frame(&mut self, display: &Display<WindowSurface>) {
        self.camera.update();
        // determine which object has been picked at the previous frame
        let picked_object = {
            let data = self.picking_pbo.read().map(|d| d[0]).unwrap_or(0);
            if data != 0 {
                self.per_instance.binary_search_by(|x| x.id.cmp(&data)).ok()
            } else {
                None
            }
        };

        self.per_instance = self.original.clone();
        if let Some(index) = picked_object {
            self.per_instance[index as usize] = PerInstance {
                id: self.per_instance[index as usize].id,
                w_position: self.per_instance[index as usize].w_position,
                color: (0.0, 1.0, 0.0)
            };
        }

        // building the uniforms
        let uniforms = uniform! {
            persp_matrix: self.camera.get_perspective(),
            view_matrix: self.camera.get_view(),
        };

        // draw parameters
        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::DepthTest::IfLess,
                write: true,
                .. Default::default()
            },
            .. Default::default()
        };

        let per_instance_buffer = glium::vertex::VertexBuffer::new(display, &self.per_instance).unwrap();

        // drawing a frame
        let mut target = display.draw();
        target.clear_color_and_depth((0.0, 0.0, 0.0, 0.0), 1.0);

        let mut target_dimensions = target.get_dimensions();
        // We need to ensure that our framebuffer is at least 1x1. Otherwise, we would get an error.
        // This could happen when minimizing or resizing the window on Windows, for example.
        if target_dimensions.0 == 0 {
            target_dimensions.0 = 1;
        }
        if target_dimensions.1 == 0 {
            target_dimensions.1 = 1;
        }

        //update picking texture
        if self.picking_attachments.is_none() || (
            self.picking_attachments.as_ref().unwrap().0.get_width(),
            self.picking_attachments.as_ref().unwrap().0.get_height().unwrap()
        ) != target.get_dimensions() {
            let (width, height) = target.get_dimensions();
            self.picking_attachments = Some((
                glium::texture::UnsignedTexture2d::empty_with_format(
                    display,
                    glium::texture::UncompressedUintFormat::U32,
                    glium::texture::MipmapsOption::NoMipmap,
                    width, height,
                ).unwrap(),
                glium::framebuffer::DepthRenderBuffer::new(
                    display,
                    glium::texture::DepthFormat::F32,
                    width, height,
                ).unwrap()
            ))
        }

        // drawing the models and pass the picking texture
        if let Some((ref picking_texture, ref depth_buffer)) = self.picking_attachments {
            //clearing the picking texture
            picking_texture.main_level().first_layer().into_image(None).unwrap().raw_clear_buffer([0u32, 0, 0, 0]);

            let mut picking_target = glium::framebuffer::SimpleFrameBuffer::with_depth_buffer(display, picking_texture, depth_buffer).unwrap();
            picking_target.clear_depth(1.0);
            picking_target.draw((&self.vertex_buffer, per_instance_buffer.per_instance().unwrap()),
                        &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                        &self.picking_program, &uniforms, &params).unwrap();
        }
        target.draw((&self.vertex_buffer, per_instance_buffer.per_instance().unwrap()),
                    &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                    &self.program, &uniforms, &params).unwrap();
        target.finish().unwrap();


        // committing into the picking pbo
        if let (Some(cursor), Some(&(ref picking_texture, _))) = (self.cursor_position, self.picking_attachments.as_ref()) {
            let read_target = glium::Rect {
                left: (cursor.0 - 1) as u32,
                bottom: picking_texture.get_height().unwrap().saturating_sub(std::cmp::max(cursor.1 - 1, 0) as u32),
                width: 1,
                height: 1,
            };

            if read_target.left < picking_texture.get_width()
            && read_target.bottom < picking_texture.get_height().unwrap() {
                picking_texture.main_level()
                    .first_layer()
                    .into_image(None).unwrap()
                    .raw_read_to_pixel_buffer(&read_target, &self.picking_pbo);
            } else {
                self.picking_pbo.write(&[0]);
            }
        } else {
            self.picking_pbo.write(&[0]);
        }
    }

    fn handle_window_event(&mut self, event: &glium::winit::event::WindowEvent, _window: &glium::winit::window::Window) {
        match event {
            glium::winit::event::WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Some(position.cast::<i32>().into());
            }
            ev => self.camera.process_input(ev),
        }
    }
}

fn main() {
    State::<Application>::run_loop();
}
