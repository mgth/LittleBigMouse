//! Persistence — port of `LittleBigMouse.Plugins.Core/Persistence`, of
//! `LittleBigMouse.Platform.Linux/JsonLayoutStore` and of `LbmPaths`.
//!
//! The storage half moves documents in and out of the user's configuration and knows
//! nothing about the layout model:
//!
//! - [`layout_dtos`]: the storage documents (`LayoutDtos.cs`), with the two shapes of
//!   a border-resistance edge in [`border_side_json`] (`BorderSideDtoJsonConverter`);
//! - [`layout_store`]: the backend contract (`ILayoutStore`), and [`json_layout_store`]
//!   its JSON implementation (`options.json`, `models.json`, `layouts/<key>.json`),
//!   whose serializer settings live in [`json_format`];
//! - [`layout_store_key`]: the name a layout is stored under (`LayoutStoreKey`, #589);
//! - [`excluded_process_defaults`] and [`excluded_list_persistence`]: the
//!   `Excluded.txt` file the daemon reads, its defaults and their one-time top-up;
//! - [`lbm_paths`]: the per-user directories;
//! - [`registry_layout_store`] and [`registry_import`]: the v5 Windows registry store,
//!   read and imported once into the JSON store (decision D2), over the real registry
//!   (`windows_registry`, Windows only) or a regedit export ([`reg_file`]).
//!
//! The engine half loads the `lbm-layout` model from a store and saves it back:
//!
//! - [`layout_persistence`]: the engine (`LayoutPersistence`) and its platform hooks;
//! - [`layout_dto_mapper`]: the model↔DTO mapping (`LayoutDtoMapper`);
//! - [`layout_migrations`]: how values written by older versions are read
//!   (`LayoutMigrations`).
//!
//! # The file format is the contract
//!
//! During the transition the C# UI keeps reading and writing the same files, so every
//! document this crate writes must mean to `System.Text.Json` exactly what the C#
//! writer's would, and every document the C# writer produces must read back here:
//! same property names, same shapes, absent members for nulls, unknown members
//! tolerated. [`json_format`] goes one step further and reproduces the C# writer's
//! bytes (indentation, escaping, number format), so a Rust save and a C# save of the
//! same data do not even differ in a diff.
//!
//! # Unknown members are kept
//!
//! The C# DTOs drop what they do not know (`System.Text.Json` skips unmapped members
//! and the DTOs have no extension data), so a C# save erases anything a newer version
//! added. Here every DTO object carries an `extra` map ([`layout_dtos::UnknownMembers`])
//! that collects the members it does not know on read and writes them back, in their
//! original order, after the known ones: a Rust writer never destroys what a newer C#
//! version wrote. This only holds for a document that goes through a read-modify-write
//! — a DTO built from scratch has nothing in `extra`, and saving it drops the unknown
//! members exactly as the C# writer does. The engine does both, as C# does: a full
//! [`LayoutPersistence::save`] builds its documents from the model, while
//! [`LayoutPersistence::save_enabled`] rewrites the stored layout document with only
//! `Enabled` changed, keeping what it does not know. The `models.json` merge keeps the
//! unknown members of the models the save does not touch.

pub mod border_side_json;
pub mod excluded_list_persistence;
pub mod excluded_process_defaults;
pub mod json_format;
pub mod json_layout_store;
pub mod layout_dto_mapper;
pub mod layout_dtos;
pub mod layout_migrations;
pub mod layout_persistence;
pub mod layout_store;
pub mod layout_store_key;
pub mod lbm_paths;
pub mod reg_file;
pub mod registry_import;
pub mod registry_layout_store;
#[cfg(windows)]
pub mod windows_registry;

pub use excluded_list_persistence::ExcludedListPersistence;
pub use json_layout_store::JsonLayoutStore;
pub use layout_dtos::{
    BorderResistanceDto, BorderSectionDto, BorderSideDto, BordersDto, GlobalOptionsDto, LayoutDto,
    LayoutOptionsDto, ModelDto, MonitorDto, SourceDto, UnknownMembers,
};
pub use layout_persistence::{LayoutPersistence, PersistencePlatform};
pub use layout_store::{LayoutStore, LayoutStoreData};
