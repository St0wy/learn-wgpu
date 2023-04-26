use std::io::{BufReader, Cursor};
use std::path::Path;

use cfg_if::cfg_if;
use wgpu::util::DeviceExt;

use crate::model::{Material, Mesh, Model, ModelVertex};
use crate::texture::Texture;

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let mut origin = location.origin().unwrap();
    origin = format!("{}/res", origin);

    let base = reqwest::Url::parse(&format!("{}/", origin)).unwrap();
    base.join(file_name).unwrap()
}

pub async fn load_string(file_name: &Path) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name.to_str().unwrap());
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("..")
                .join("..")
                .join("..")
                .join("res")
                .join(file_name);
            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

pub async fn load_binary(file_name: &Path) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name.to_str().unwrap());
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("..")
                .join("..")
                .join("..")
                .join("res")
                .join(file_name);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}

pub async fn load_texture(
    file_name: &Path,
    is_normal_map: bool,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = load_binary(file_name).await?;
    Texture::from_bytes(
        device,
        queue,
        &data,
        file_name.to_str().unwrap(),
        is_normal_map,
    )
}

pub async fn load_model(
    file_name: &Path,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
    let obj_text = load_string(file_name).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let material_path = file_name.parent().unwrap().join(p);
            let mat_text = load_string(&material_path).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_texture_path = file_name.parent().unwrap().join(m.diffuse_texture);
        let diffuse_texture = load_texture(&diffuse_texture_path, false, device, queue).await?;

        let normal_texture_path = file_name.parent().unwrap().join(m.normal_texture);
        let normal_texture = load_texture(&normal_texture_path, true, device, queue).await?;

        materials.push(Material::new(
            device,
            &m.name,
            diffuse_texture,
            normal_texture,
            layout,
        ))
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let mut vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                    tangent: [0.0; 3],
                    bitangent: [0.0; 3],
                })
                .collect::<Vec<_>>();

            let indices = &m.mesh.indices;
            let mut triangles_included = vec![0; vertices.len()];

            for chunk in indices.chunks(3) {
                let v0 = vertices[chunk[0] as usize];
                let v1 = vertices[chunk[1] as usize];
                let v2 = vertices[chunk[2] as usize];

                let pos0: cgmath::Vector3<f32> = v0.position.into();
                let pos1: cgmath::Vector3<f32> = v1.position.into();
                let pos2: cgmath::Vector3<f32> = v2.position.into();

                let uv0: cgmath::Vector2<f32> = v0.tex_coords.into();
                let uv1: cgmath::Vector2<f32> = v1.tex_coords.into();
                let uv2: cgmath::Vector2<f32> = v2.tex_coords.into();

                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                let bitangent = (delta_pos2 * delta_uv2.x - delta_pos1 * delta_uv2.x) * -r;

                vertices[chunk[0] as usize].tangent =
                    (tangent + cgmath::Vector3::from(vertices[chunk[0] as usize].tangent)).into();
                vertices[chunk[1] as usize].tangent =
                    (tangent + cgmath::Vector3::from(vertices[chunk[1] as usize].tangent)).into();
                vertices[chunk[2] as usize].tangent =
                    (tangent + cgmath::Vector3::from(vertices[chunk[2] as usize].tangent)).into();
                vertices[chunk[0] as usize].bitangent = (bitangent
                    + cgmath::Vector3::from(vertices[chunk[0] as usize].bitangent))
                .into();
                vertices[chunk[1] as usize].bitangent = (bitangent
                    + cgmath::Vector3::from(vertices[chunk[1] as usize].bitangent))
                .into();
                vertices[chunk[2] as usize].bitangent = (bitangent
                    + cgmath::Vector3::from(vertices[chunk[2] as usize].bitangent))
                .into();

                triangles_included[chunk[0] as usize] += 1;
                triangles_included[chunk[1] as usize] += 1;
                triangles_included[chunk[2] as usize] += 1;
            }

            for (i, n) in triangles_included.into_iter().enumerate() {
                let denominator = 1.0 / n as f32;
                let mut vertex = &mut vertices[i];
                vertex.tangent = (cgmath::Vector3::from(vertex.tangent) * denominator).into();
                vertex.bitangent = (cgmath::Vector3::from(vertex.bitangent) * denominator).into();
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", file_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", file_name)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            Mesh {
                name: file_name.to_str().unwrap().to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<Mesh>>();

    Ok(Model { meshes, materials })
}
