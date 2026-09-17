//! Adapt the real engine's sweep to the frontend JSON contract.
use lbm_ipc::probe::{Edge, Report, Run, Side, Zone};

pub fn report(layout_id: String, layout_revision: u64, xml: &str) -> Result<Report, String> {
    let layout = lbm_zones::ZonesLayout::from_xml(xml).ok_or("invalid layout to probe")?;
    let zones = lbm_engine::probe::probe(layout)
        .into_iter()
        .map(|zone| {
            let edges = zone
                .edges
                .into_iter()
                .map(|edge| {
                    let side = match edge.side {
                        "Left" => Side::Left,
                        "Right" => Side::Right,
                        "Top" => Side::Top,
                        "Bottom" => Side::Bottom,
                        other => return Err(format!("unknown probe side: {other}")),
                    };
                    Ok(Edge {
                        side,
                        runs: edge
                            .runs
                            .into_iter()
                            .map(|run| Run {
                                from: run.from,
                                to: run.to,
                                target: (run.target >= 0).then_some(run.target),
                            })
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Zone {
                id: zone.id,
                name: zone.name,
                device_id: zone.device_id,
                edges,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Report {
        layout_id,
        layout_revision,
        zones,
    })
}
