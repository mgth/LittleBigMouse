//! The whole model↔DTO mapping — port of `Persistence/LayoutDtoMapper.cs`.
//!
//! The `apply_*` functions read a DTO into the live model (a `None` field is absent
//! from the store: the live value is kept), the `to_*` functions write the model back.
//! Both directions stay in this one file on purpose: adding a persisted field means one
//! DTO member ([`layout_dtos`](crate::layout_dtos)) and its two mapping lines here.
//!
//! Nothing here interprets history: reading a value that means something different
//! depending on when it was written goes through [`layout_migrations`].
//!
//! The model is changed through the layout's edit functions, one per C# property
//! write, so every write has the side effects it has in C# (saved flags, border
//! mirroring, republished values). The engine marks everything saved after a load.

use indexmap::IndexMap;
use lbm_layout::geo::{Point, Rect, Size, Thickness};
use lbm_layout::model::{
    BorderSection, BorderSide, Layout, LayoutOptions, Monitor, MonitorModel, Ratio,
};

use crate::layout_dtos::{
    BorderResistanceDto, BorderSectionDto, BorderSideDto, BordersDto, GlobalOptionsDto, LayoutDto,
    LayoutOptionsDto, ModelDto, MonitorDto, SourceDto,
};
use crate::layout_migrations;

//==================//
// DTO -> model     //
//==================//

/// `o.X = dto.X ?? o.X`.
fn keep<T: Clone>(field: &mut T, stored: &Option<T>) {
    if let Some(value) = stored {
        *field = value.clone();
    }
}

/// C# `Apply(ILayoutOptions, GlobalOptionsDto?)`: the app-level options.
pub fn apply_global_options(o: &mut LayoutOptions, dto: Option<&GlobalOptionsDto>) {
    let Some(dto) = dto else { return };

    keep(&mut o.priority, &dto.priority);
    keep(&mut o.priority_unhooked, &dto.priority_unhooked);
    keep(&mut o.home_cinema, &dto.home_cinema);
    keep(&mut o.pinned, &dto.pinned);
    keep(&mut o.auto_update, &dto.auto_update);
    keep(&mut o.start_minimized, &dto.start_minimized);
    keep(&mut o.start_elevated, &dto.start_elevated);
    keep(&mut o.debug_tools, &dto.debug_tools);
    keep(&mut o.experimental_features, &dto.experimental_features);
    keep(&mut o.vcp_control, &dto.vcp_control);
    keep(
        &mut o.show_monitor_action_warning,
        &dto.show_monitor_action_warning,
    );
    keep(&mut o.border_values, &dto.border_values);
    keep(&mut o.rescue_shortcut, &dto.rescue_shortcut);
    keep(&mut o.hide_tray_icon, &dto.hide_tray_icon);
}

/// C# `Apply(ILayoutOptions, LayoutOptionsDto?)`: the per-layout options.
pub fn apply_layout_options(o: &mut LayoutOptions, dto: Option<&LayoutOptionsDto>) {
    let Some(dto) = dto else { return };

    keep(&mut o.allow_overlaps, &dto.allow_overlaps);
    keep(&mut o.allow_discontinuity, &dto.allow_discontinuity);
    keep(&mut o.algorithm, &dto.algorithm);
    keep(&mut o.minimal_edge_overlap, &dto.minimal_edge_overlap);
    keep(&mut o.max_travel_distance, &dto.max_travel_distance);
    keep(&mut o.freelook_check_interval, &dto.freelook_check_interval);
    keep(&mut o.freelook_enabled, &dto.freelook_enabled);
    keep(&mut o.loop_x, &dto.loop_x);
    keep(&mut o.loop_y, &dto.loop_y);
    keep(&mut o.enabled, &dto.enabled);
    keep(&mut o.adjust_pointer, &dto.adjust_pointer);
    keep(&mut o.adjust_speed, &dto.adjust_speed);

    // READ-ONLY legacy: the app-level values are applied first, and a layout that still
    // carries its own overrides them so nobody's priority changes on upgrade. Nothing
    // writes them back (see `to_layout_options_dto`), so this ends at the first save.
    keep(&mut o.priority, &dto.priority);
    keep(&mut o.priority_unhooked, &dto.priority_unhooked);
}

/// C# `Apply(PhysicalMonitorModel, ModelDto)`: a model's stored size, borders and name.
pub fn apply_model(layout: &mut Layout, pnp_code: &str, dto: &ModelDto) {
    layout.edit_model(pnp_code, |size, name| {
        let fixed_ratio = size.fixed_aspect_ratio();
        size.set_fixed_aspect_ratio(false);

        let borders = dto.borders.as_ref();
        size.set_top_border(borders.and_then(|b| b.top).unwrap_or(size.borders().top));
        size.set_right_border(
            borders
                .and_then(|b| b.right)
                .unwrap_or(size.borders().right),
        );
        size.set_bottom_border(
            borders
                .and_then(|b| b.bottom)
                .unwrap_or(size.borders().bottom),
        );
        size.set_left_border(borders.and_then(|b| b.left).unwrap_or(size.borders().left));

        // Placeholder sizes (#419) and pre-5.4.1 oriented sizes (#507).
        let (width, height) = layout_migrations::stored_model_size(
            size.width(),
            size.height(),
            dto.width,
            dto.height,
        );
        if let Some(w) = width {
            size.set_width(w);
        }
        if let Some(h) = height {
            size.set_height(h);
        }

        size.set_fixed_aspect_ratio(fixed_ratio);

        if let Some(pnp_name) = dto.pnp_name.as_ref().filter(|n| !n.is_empty()) {
            *name = Some(pnp_name.clone());
        }
    });
}

/// C# `Apply(PhysicalMonitor, MonitorDto)`: a monitor's stored placement, sources,
/// border resistance, exclusion and own borders.
pub fn apply_monitor(layout: &mut Layout, id: &str, dto: &MonitorDto) {
    let Some(sources) = layout.monitor(id).map(|m| m.sources.clone()) else {
        return;
    };
    for source_id in &sources {
        let Some(stored) = dto.sources.as_ref().and_then(|s| s.get(source_id)) else {
            continue;
        };
        let Some(source) = layout.source(source_id).map(|s| s.source.clone()) else {
            continue;
        };

        // Detached sources restore their stored pixel geometry (nothing current to
        // keep); attached ones keep the live geometry, the store is just a backup.
        if !source.attached_to_desktop {
            let px = source.in_pixel;
            layout.set_source_in_pixel(
                source_id,
                Rect::from_location_size(
                    Point::new(
                        stored.pixel_x.unwrap_or(px.x),
                        stored.pixel_y.unwrap_or(px.y),
                    ),
                    Size::new(
                        stored.pixel_width.unwrap_or(px.width),
                        stored.pixel_height.unwrap_or(px.height),
                    ),
                ),
            );
            layout.set_source_orientation(
                source_id,
                stored.orientation.unwrap_or(source.orientation),
            );
        }

        if dto.active_source.as_deref() == Some(source_id.as_str()) {
            layout.set_active_source(id, Some(source_id.clone()));
        }
    }

    if let Some(x) = dto.x_location_in_mm {
        let y = location(layout, id).y;
        layout.set_location(id, Point::new(x, y));
        layout.set_placed(id, true);
    }
    if let Some(y) = dto.y_location_in_mm {
        let x = location(layout, id).x;
        layout.set_location(id, Point::new(x, y));
        layout.set_placed(id, true);
    }

    let ratio = layout
        .monitor(id)
        .map_or(Ratio::uniform(1.0), |m| m.depth_ratio);
    layout.set_depth_ratio(
        id,
        Ratio::new(
            dto.physical_ratio_x.unwrap_or(ratio.x),
            dto.physical_ratio_y.unwrap_or(ratio.y),
        ),
    );

    // The edge length is needed to convert a stored whole-edge resistance into the
    // section that now expresses it. C# reads it off the depth projection, which a
    // monitor without an active source does not have (C# would throw there): no
    // length, and the legacy value is dropped.
    let (across_mm, down_mm) = layout
        .monitor(id)
        .and_then(|m| layout.depth_projection(m))
        .map_or((0.0, 0.0), |dp| (dp.width, dp.height));

    let resistance = dto.border_resistance.as_ref();
    layout.edit_border_resistance(id, |br| {
        apply_side(
            &mut br.left,
            resistance.and_then(|r| r.left.as_ref()),
            down_mm,
        );
        apply_side(
            &mut br.top,
            resistance.and_then(|r| r.top.as_ref()),
            across_mm,
        );
        apply_side(
            &mut br.right,
            resistance.and_then(|r| r.right.as_ref()),
            down_mm,
        );
        apply_side(
            &mut br.bottom,
            resistance.and_then(|r| r.bottom.as_ref()),
            across_mm,
        );
    });

    if let Some(excluded) = dto.excluded_from_layout {
        layout.set_excluded(id, excluded);
    }

    // Per-monitor bezel borders load whatever the current mode is, so switching to
    // PerMonitor is live (no restart required). Stored values only exist once the user
    // edited them in PerMonitor mode: until then the borders keep mirroring the live
    // model values, so the FIRST switch starts from the monitor's current PerModel
    // borders.
    if let Some(b) = &dto.borders {
        let current = layout
            .monitor(id)
            .map_or(Thickness::default(), Monitor::borders);
        layout.set_monitor_borders(
            id,
            Thickness::new(
                b.left.unwrap_or(current.left),
                b.top.unwrap_or(current.top),
                b.right.unwrap_or(current.right),
                b.bottom.unwrap_or(current.bottom),
            ),
        );
        layout.set_borders_customized(id, true);
    }
}

fn location(layout: &Layout, id: &str) -> Point {
    layout
        .monitor(id)
        .map_or(Point::new(0.0, 0.0), Monitor::location)
}

/// C# `Apply(BorderSide, BorderSideDto?, double)`: an edge's sections, a stored
/// whole-edge resistance migrated into one (see [`layout_migrations::sections`]).
pub fn apply_side(side: &mut BorderSide, dto: Option<&BorderSideDto>, edge_length_mm: f64) {
    let Some(dto) = dto else { return };

    side.sections = layout_migrations::sections(dto, edge_length_mm)
        .iter()
        .map(|s| {
            BorderSection::new(
                s.from.unwrap_or(0.0),
                s.to.unwrap_or(0.0),
                s.r#move.unwrap_or(0.0),
                s.move_block.unwrap_or(false),
                s.drag.unwrap_or(0.0),
                s.drag_block.unwrap_or(false),
            )
        })
        .collect();
}

//==================//
// model -> DTO     //
//==================//

/// C# `ToGlobalOptionsDto`: the app-level options. `excluded_defaults_version` is not
/// part of the options model: it is read at load time and round-tripped through every
/// write so the one-time exclusion-list top-up stays one-time.
pub fn to_global_options_dto(
    o: &LayoutOptions,
    excluded_defaults_version: Option<i32>,
) -> GlobalOptionsDto {
    GlobalOptionsDto {
        priority: Some(o.priority.clone()),
        priority_unhooked: Some(o.priority_unhooked.clone()),
        home_cinema: Some(o.home_cinema),
        pinned: Some(o.pinned),
        auto_update: Some(o.auto_update),
        start_minimized: Some(o.start_minimized),
        start_elevated: Some(o.start_elevated),
        debug_tools: Some(o.debug_tools),
        experimental_features: Some(o.experimental_features),
        vcp_control: Some(o.vcp_control),
        show_monitor_action_warning: Some(o.show_monitor_action_warning),
        border_values: Some(o.border_values.clone()),
        rescue_shortcut: Some(o.rescue_shortcut.clone()),
        hide_tray_icon: Some(o.hide_tray_icon),
        excluded_defaults_version,
        ..Default::default()
    }
}

/// C# `ToLayoutDto`: the layout document, its monitors in the layout's order.
pub fn to_layout_dto(layout: &Layout) -> LayoutDto {
    LayoutDto {
        options: Some(to_layout_options_dto(&layout.options)),
        monitors: layout
            .monitors()
            .iter()
            .map(|m| (m.id.clone(), to_monitor_dto(layout, m)))
            .collect(),
        ..Default::default()
    }
}

/// C# `ToDto(ILayoutOptions)`: the per-layout options.
///
/// Priority/PriorityUnhooked are deliberately absent: they are app-level options that
/// once lived here too, and both locations load into the SAME options property. Writing
/// the property back to both made a layout's leftover value the app-level one at the
/// next save, and gave a copy of it to every other layout after that. They are written
/// to the app level only; the layout copy is read as a fallback and then dropped.
pub fn to_layout_options_dto(o: &LayoutOptions) -> LayoutOptionsDto {
    LayoutOptionsDto {
        allow_overlaps: Some(o.allow_overlaps),
        allow_discontinuity: Some(o.allow_discontinuity),
        algorithm: Some(o.algorithm.clone()),
        minimal_edge_overlap: Some(o.minimal_edge_overlap),
        max_travel_distance: Some(o.max_travel_distance),
        freelook_check_interval: Some(o.freelook_check_interval),
        freelook_enabled: Some(o.freelook_enabled),
        loop_x: Some(o.loop_x),
        loop_y: Some(o.loop_y),
        enabled: Some(o.enabled),
        adjust_pointer: Some(o.adjust_pointer),
        adjust_speed: Some(o.adjust_speed),
        ..Default::default()
    }
}

/// C# `ToDto(BorderSide)`: sections only. The per-edge resistance is gone, and writing
/// it back would resurrect it on the next load through the migration path. The side is
/// always emitted, even empty, so deleting the last section clears what the store
/// still holds.
pub fn to_side_dto(side: &BorderSide) -> BorderSideDto {
    BorderSideDto {
        sections: (!side.sections.is_empty()).then(|| {
            side.sections
                .iter()
                .map(|s| BorderSectionDto {
                    from: Some(s.from()),
                    to: Some(s.to()),
                    r#move: Some(s.move_resistance()),
                    move_block: Some(s.move_block()),
                    drag: Some(s.drag()),
                    drag_block: Some(s.drag_block()),
                    ..Default::default()
                })
                .collect()
        }),
        ..Default::default()
    }
}

/// C# `ToDto(PhysicalMonitor)`: a monitor's placement, resistance, own borders and
/// attached sources.
pub fn to_monitor_dto(layout: &Layout, monitor: &Monitor) -> MonitorDto {
    let location = monitor.location();
    let br = &monitor.border_resistance;
    let borders = monitor.borders();
    MonitorDto {
        x_location_in_mm: Some(location.x),
        y_location_in_mm: Some(location.y),
        physical_ratio_x: Some(monitor.depth_ratio.x),
        physical_ratio_y: Some(monitor.depth_ratio.y),
        border_resistance: Some(BorderResistanceDto {
            left: Some(to_side_dto(&br.left)),
            top: Some(to_side_dto(&br.top)),
            right: Some(to_side_dto(&br.right)),
            bottom: Some(to_side_dto(&br.bottom)),
            ..Default::default()
        }),
        // Stored whatever the current mode is (they must survive a save made in
        // PerModel mode), but only once the monitor owns them: uncustomized monitors
        // keep mirroring the model and store nothing.
        borders: monitor.borders_customized().then(|| BordersDto {
            left: Some(borders.left),
            top: Some(borders.top),
            right: Some(borders.right),
            bottom: Some(borders.bottom),
            ..Default::default()
        }),
        active_source: monitor.active_source.clone(),
        serial_number: monitor.serial_number.clone(),
        excluded_from_layout: Some(monitor.excluded_from_layout),
        sources: Some(
            monitor
                .sources
                .iter()
                .filter_map(|id| layout.source(id))
                .filter(|s| s.source.attached_to_desktop)
                .map(|s| {
                    let d = &s.source;
                    let dto = SourceDto {
                        pixel_x: Some(d.in_pixel.x),
                        pixel_y: Some(d.in_pixel.y),
                        pixel_width: Some(d.in_pixel.width),
                        pixel_height: Some(d.in_pixel.height),
                        orientation: Some(d.orientation),
                        display_name: d.display_name.clone(),
                        primary: Some(d.primary),
                        ..Default::default()
                    };
                    (d.id.clone(), dto)
                })
                .collect(),
        ),
        ..Default::default()
    }
}

/// C# `ToDto(PhysicalMonitorModel)`: a model's size, borders and name.
pub fn to_model_dto(model: &MonitorModel) -> ModelDto {
    let size = &model.physical_size;
    let borders = size.borders();
    ModelDto {
        width: Some(size.width()),
        height: Some(size.height()),
        borders: Some(BordersDto {
            left: Some(borders.left),
            top: Some(borders.top),
            right: Some(borders.right),
            bottom: Some(borders.bottom),
            ..Default::default()
        }),
        pnp_name: model.pnp_device_name.clone(),
        ..Default::default()
    }
}

/// The models of a layout's monitors, once each, in the order the monitors first use
/// them (C#: `Select(m => m.Model).DistinctBy(m => m.PnpCode).ToDictionary(...)`).
pub fn to_model_dtos(layout: &Layout) -> IndexMap<String, ModelDto> {
    let mut models = IndexMap::new();
    for monitor in layout.monitors() {
        if !models.contains_key(&monitor.model) {
            if let Some(model) = layout.model(&monitor.model) {
                models.insert(monitor.model.clone(), to_model_dto(model));
            }
        }
    }
    models
}
