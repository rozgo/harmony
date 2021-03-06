use crate::{
    graphics::{
        material::{skybox::SkyboxType, Skybox}, pipelines::SkyboxUniforms,
        CommandBufferQueue, CommandQueueItem, renderer::DepthTexture,
        resources::{CurrentRenderTarget, GPUResourceManager}, pipeline_manager::{PipelineManager, Pipeline}
    },
    scene::components::CameraData,
};
use legion::prelude::*;
use std::sync::Arc;

pub fn create() -> Box<dyn Schedulable> {
    SystemBuilder::new("render_skybox")
        .write_resource::<CommandBufferQueue>()
        .read_resource::<CurrentRenderTarget>()
        .read_resource::<GPUResourceManager>()
        .read_resource::<PipelineManager>()
        .read_resource::<wgpu::Device>()
        .read_resource::<Arc<wgpu::SwapChainOutput>>()
        .read_resource::<DepthTexture>()
        .with_query(<(Read<Skybox>,)>::query())
        .with_query(<(Read<CameraData>,)>::query())
        .build(
            |_,
             world,
             (command_buffer_queue, current_render_target, resource_manager, pipeline_manager, device, output, depth_texture),
             (skyboxes, cameras)| {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("skybox_clear_pass"),
                });

                let pipeline: &Pipeline = pipeline_manager.get("skybox", None).unwrap();
               
                let global_bind_group = resource_manager.get_bind_group("skybox_globals", 0).unwrap();
                let uniform_buffer = resource_manager.get_buffer("skybox_buffer");
                
                for (camera_data,) in cameras.iter(&world) {
                    if camera_data.active {
                        let uniforms = SkyboxUniforms {
                            proj: camera_data.projection,
                            view: camera_data.view,
                        };

                        let constants_buffer = device.create_buffer_with_data(
                            bytemuck::bytes_of(&uniforms),
                            wgpu::BufferUsage::COPY_SRC,
                        );

                        encoder.copy_buffer_to_buffer(
                            &constants_buffer,
                            0,
                            uniform_buffer,
                            0,
                            std::mem::size_of::<SkyboxUniforms>() as u64,
                        );
                    }
                }

                let view_attachment = if current_render_target.0.is_some() {
                    &current_render_target.0.as_ref().unwrap().1
                } else {
                    &output.view
                };

                let depth_attachment = if current_render_target.0.is_some() {
                    current_render_target.0.as_ref().unwrap().0.depth_texture_view.as_ref().unwrap()
                } else {
                    &depth_texture.0
                };

                for (skybox,) in skyboxes.iter(&world) {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: view_attachment,
                            resolve_target: None,
                            load_op: wgpu::LoadOp::Clear,
                            store_op: wgpu::StoreOp::Store,
                            clear_color: wgpu::Color {
                                r: skybox.clear_color.x as f64,
                                g: skybox.clear_color.y as f64,
                                b: skybox.clear_color.z as f64,
                                a: 1.0,
                            },
                        }],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                            attachment: depth_attachment,
                            depth_load_op: wgpu::LoadOp::Clear,
                            depth_store_op: wgpu::StoreOp::Store,
                            stencil_load_op: wgpu::LoadOp::Clear,
                            stencil_store_op: wgpu::StoreOp::Store,
                            clear_depth: 1.0,
                            clear_stencil: 0,
                        }),
                    });

                    if skybox.skybox_type == SkyboxType::HdrCubemap {
                        render_pass.set_pipeline(&pipeline.render_pipeline);
                        render_pass.set_bind_group(0, &global_bind_group.group, &[]);

                        render_pass.set_bind_group(1, skybox.cubemap_bind_group.as_ref().unwrap(), &[]);
                        render_pass.draw(0..3 as u32, 0..1);
                    }
                }
                
                command_buffer_queue
                    .push(CommandQueueItem {
                        buffer: encoder.finish(),
                        name: "skybox".to_string(),
                    })
                    .unwrap();
            },
        )
}
