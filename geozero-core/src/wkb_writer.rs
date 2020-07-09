use crate::wkb_common::{WKBByteOrder, WKBGeometryType};
use geozero::error::Result;
use geozero::{CoordDimensions, FeatureProcessor, GeomProcessor, PropertyProcessor};
use scroll::IOwrite;
use std::io::Write;

pub struct WkbWriter<'a, W: Write> {
    pub dims: CoordDimensions,
    pub srid: Option<i32>,
    endian: scroll::Endian,
    first_header: bool,
    geom_state: GeomState,
    out: &'a mut W,
}

#[derive(PartialEq, Debug)]
enum GeomState {
    Normal,
    RingGeom,
    MultiPointGeom,
}

impl<'a, W: Write> WkbWriter<'a, W> {
    pub fn new(out: &'a mut W) -> WkbWriter<'a, W> {
        WkbWriter {
            dims: CoordDimensions::default(),
            endian: scroll::LE,
            srid: None,
            first_header: true,
            geom_state: GeomState::Normal,
            out,
        }
    }

    // Write header in selected format
    fn write_header(&mut self, wkb_type: WKBGeometryType) -> Result<()> {
        self.write_ewkb_header(wkb_type.clone())?;
        Ok(())
    }
    // OGC WKB header
    fn write_wkb_header(&mut self, wkb_type: WKBGeometryType) -> Result<()> {
        let byte_order = if self.endian == scroll::BE {
            WKBByteOrder::XDR
        } else {
            WKBByteOrder::NDR
        };
        self.out.iowrite(byte_order as u8)?;
        self.out.iowrite_with(wkb_type as u32, self.endian)?;
        Ok(())
    }

    /// EWKB header according to https://git.osgeo.org/gitea/postgis/postgis/src/branch/master/doc/ZMSgeoms.txt
    fn write_ewkb_header(&mut self, wkb_type: WKBGeometryType) -> Result<()> {
        let byte_order = if self.endian == scroll::BE {
            WKBByteOrder::XDR
        } else {
            WKBByteOrder::NDR
        };
        self.out.iowrite(byte_order as u8)?;

        let mut type_id = wkb_type as u32;
        if self.dims.z {
            type_id |= 0x80000000;
        }
        if self.dims.m {
            type_id |= 0x40000000;
        }
        if self.srid.is_some() && self.first_header {
            type_id |= 0x20000000;
        }
        self.out.iowrite_with(type_id, self.endian)?;

        if self.first_header {
            // write SRID in main header only
            if let Some(srid) = self.srid {
                self.out.iowrite_with(srid, self.endian)?;
            }
            self.first_header = false;
        }

        Ok(())
    }

    /// GPKG geometry header according to http://www.geopackage.org/spec/#gpb_format
    fn write_gpkg_header(&mut self) -> Result<()> {
        let magic = b"GP";
        self.out.write(magic)?;
        let version: u8 = 1; // TODO
        self.out.iowrite(version)?;
        // let flags = self.out.ioread::<u8>()?;
        // // println!("flags: {:#010b}", flags);
        // let _extended = (flags & 0b0010_0000) >> 5 == 1;
        // let _empty = (flags & 0b0001_0000) >> 4 == 1;
        // let env_len = match (flags & 0b0000_1110) >> 1 {
        //     0 => 0,
        //     1 => 4,
        //     2 => 6,
        //     3 => 6,
        //     4 => 8,
        //     _ => {
        //         return Err(GeozeroError::GeometryFormat);
        //     }
        // };
        // let endian = if flags & 0b0000_0001 == 0 {
        //     scroll::BE
        // } else {
        //     scroll::LE
        // };
        // let srid = self.out.ioread_with::<i32>(endian)?;
        // let envelope: std::result::Result<Vec<f64>, _> = (0..env_len)
        //     .map(|_| self.out.ioread_with::<f64>(endian))
        //     .collect();
        // let envelope = envelope?;
        Ok(())
    }
}

impl<W: Write> GeomProcessor for WkbWriter<'_, W> {
    fn dimensions(&self) -> CoordDimensions {
        self.dims
    }
    fn xy(&mut self, x: f64, y: f64, _idx: usize) -> Result<()> {
        if self.geom_state == GeomState::MultiPointGeom {
            self.write_header(WKBGeometryType::Point)?;
        }
        self.out.iowrite_with(x, self.endian)?;
        self.out.iowrite_with(y, self.endian)?;
        Ok(())
    }
    fn coordinate(
        &mut self,
        x: f64,
        y: f64,
        z: Option<f64>,
        m: Option<f64>,
        _t: Option<f64>,
        _tm: Option<u64>,
        _idx: usize,
    ) -> Result<()> {
        if self.geom_state == GeomState::MultiPointGeom {
            self.write_header(WKBGeometryType::Point)?;
        }
        self.out.iowrite_with(x, self.endian)?;
        self.out.iowrite_with(y, self.endian)?;
        if let Some(z) = z {
            self.out.iowrite_with(z, self.endian)?;
        }
        if let Some(m) = m {
            self.out.iowrite_with(m, self.endian)?;
        }
        Ok(())
    }
    fn point_begin(&mut self, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::Point)
    }
    fn multipoint_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::MultiPoint)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        self.geom_state = GeomState::MultiPointGeom;
        Ok(())
    }
    fn multipoint_end(&mut self, _idx: usize) -> Result<()> {
        self.geom_state = GeomState::Normal;
        Ok(())
    }
    fn linestring_begin(&mut self, _tagged: bool, size: usize, _idx: usize) -> Result<()> {
        if self.geom_state != GeomState::RingGeom {
            self.write_header(WKBGeometryType::LineString)?;
        }
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn multilinestring_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::MultiLineString)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn polygon_begin(&mut self, _tagged: bool, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::Polygon)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        self.geom_state = GeomState::RingGeom;
        Ok(())
    }
    fn polygon_end(&mut self, _tagged: bool, _idx: usize) -> Result<()> {
        self.geom_state = GeomState::Normal;
        Ok(())
    }
    fn multipolygon_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::MultiPolygon)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn geometrycollection_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::GeometryCollection)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }

    fn circularstring_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::CircularString)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn compoundcurve_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::CompoundCurve)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn curvepolygon_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::CurvePolygon)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn multicurve_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::MultiCurve)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn multisurface_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::MultiSurface)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }

    fn triangle_begin(&mut self, _tagged: bool, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::Triangle)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        self.geom_state = GeomState::RingGeom;
        Ok(())
    }
    fn triangle_end(&mut self, _tagged: bool, _idx: usize) -> Result<()> {
        self.geom_state = GeomState::Normal;
        Ok(())
    }
    fn polyhedralsurface_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::PolyhedralSurface)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
    fn tin_begin(&mut self, size: usize, _idx: usize) -> Result<()> {
        self.write_header(WKBGeometryType::Tin)?;
        self.out.iowrite_with(size as u32, self.endian)?;
        Ok(())
    }
}

impl<W: Write> PropertyProcessor for WkbWriter<'_, W> {}

impl<W: Write> FeatureProcessor for WkbWriter<'_, W> {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::wkb::process_ewkb_geom;

    fn ewkb_roundtrip(ewkbstr: &str, with_z: bool, srid: Option<i32>) -> bool {
        let wkb_in = hex::decode(ewkbstr).unwrap();
        let mut wkb_out: Vec<u8> = Vec::new();
        let mut writer = WkbWriter::new(&mut wkb_out);
        writer.dims.z = with_z;
        writer.srid = srid;
        assert!(process_ewkb_geom(&mut wkb_in.as_slice(), &mut writer).is_ok());
        let ok = wkb_out == wkb_in;
        if !ok {
            dbg!(hex::encode(&wkb_out));
        }
        ok
    }

    #[test]
    fn ewkb_geometries() {
        // SELECT 'POINT(10 -20)'::geometry
        assert!(ewkb_roundtrip(
            "0101000000000000000000244000000000000034C0",
            false,
            None
        ));

        // SELECT 'SRID=4326;MULTIPOINT (10 -20 100, 0 -0.5 101)'::geometry
        assert!(ewkb_roundtrip("01040000A0E6100000020000000101000080000000000000244000000000000034C0000000000000594001010000800000000000000000000000000000E0BF0000000000405940", true, Some(4326)));

        // SELECT 'SRID=4326;LINESTRING (10 -20 100, 0 -0.5 101)'::geometry
        assert!(ewkb_roundtrip("01020000A0E610000002000000000000000000244000000000000034C000000000000059400000000000000000000000000000E0BF0000000000405940", true, Some(4326)));

        // SELECT 'SRID=4326;MULTILINESTRING ((10 -20, 0 -0.5), (0 0, 2 0))'::geometry
        assert!(ewkb_roundtrip("0105000020E610000002000000010200000002000000000000000000244000000000000034C00000000000000000000000000000E0BF0102000000020000000000000000000000000000000000000000000000000000400000000000000000", false, Some(4326)));

        // SELECT 'SRID=4326;POLYGON ((0 0, 2 0, 2 2, 0 2, 0 0))'::geometry
        assert!(ewkb_roundtrip("0103000020E610000001000000050000000000000000000000000000000000000000000000000000400000000000000000000000000000004000000000000000400000000000000000000000000000004000000000000000000000000000000000", false, Some(4326)));

        // SELECT 'SRID=4326;MULTIPOLYGON (((0 0, 2 0, 2 2, 0 2, 0 0)), ((10 10, -2 10, -2 -2, 10 -2, 10 10)))'::geometry
        assert!(ewkb_roundtrip("0106000020E610000002000000010300000001000000050000000000000000000000000000000000000000000000000000400000000000000000000000000000004000000000000000400000000000000000000000000000004000000000000000000000000000000000010300000001000000050000000000000000002440000000000000244000000000000000C0000000000000244000000000000000C000000000000000C0000000000000244000000000000000C000000000000024400000000000002440", false, Some(4326)));

        // SELECT 'GeometryCollection(POINT (10 10),POINT (30 30),LINESTRING (15 15, 20 20))'::geometry
        assert!(ewkb_roundtrip("01070000000300000001010000000000000000002440000000000000244001010000000000000000003E400000000000003E400102000000020000000000000000002E400000000000002E4000000000000034400000000000003440", false, None));
    }

    #[test]
    fn ewkb_curves() {
        // SELECT 'CIRCULARSTRING(0 0,1 1,2 0)'::geometry
        assert!(ewkb_roundtrip("01080000000300000000000000000000000000000000000000000000000000F03F000000000000F03F00000000000000400000000000000000", false, None));

        // SELECT 'COMPOUNDCURVE (CIRCULARSTRING (0 0,1 1,2 0),(2 0,3 0))'::geometry
        assert!(ewkb_roundtrip("01090000000200000001080000000300000000000000000000000000000000000000000000000000F03F000000000000F03F000000000000004000000000000000000102000000020000000000000000000040000000000000000000000000000008400000000000000000", false, None));

        // SELECT 'CURVEPOLYGON(COMPOUNDCURVE(CIRCULARSTRING(0 0,1 1,2 0),(2 0,3 0,3 -1,0 -1,0 0)))'::geometry
        assert!(ewkb_roundtrip("010A0000000100000001090000000200000001080000000300000000000000000000000000000000000000000000000000F03F000000000000F03F0000000000000040000000000000000001020000000500000000000000000000400000000000000000000000000000084000000000000000000000000000000840000000000000F0BF0000000000000000000000000000F0BF00000000000000000000000000000000", false, None));

        // SELECT 'MULTICURVE((0 0, 5 5),CIRCULARSTRING(4 0, 4 4, 8 4))'::geometry
        assert!(ewkb_roundtrip("010B000000020000000102000000020000000000000000000000000000000000000000000000000014400000000000001440010800000003000000000000000000104000000000000000000000000000001040000000000000104000000000000020400000000000001040", false, None));

        // SELECT 'MULTISURFACE (CURVEPOLYGON (COMPOUNDCURVE (CIRCULARSTRING (0 0,1 1,2 0),(2 0,3 0,3 -1,0 -1,0 0))))'::geometry
        assert!(ewkb_roundtrip("010C00000001000000010A0000000100000001090000000200000001080000000300000000000000000000000000000000000000000000000000F03F000000000000F03F0000000000000040000000000000000001020000000500000000000000000000400000000000000000000000000000084000000000000000000000000000000840000000000000F0BF0000000000000000000000000000F0BF00000000000000000000000000000000", false, None));
    }

    #[test]
    fn ewkb_surfaces() {
        // SELECT 'POLYHEDRALSURFACE(((0 0 0,0 0 1,0 1 1,0 1 0,0 0 0)),((0 0 0,0 1 0,1 1 0,1 0 0,0 0 0)),((0 0 0,1 0 0,1 0 1,0 0 1,0 0 0)),((1 1 0,1 1 1,1 0 1,1 0 0,1 1 0)),((0 1 0,0 1 1,1 1 1,1 1 0,0 1 0)),((0 0 1,1 0 1,1 1 1,0 1 1,0 0 1)))'::geometry
        assert!(ewkb_roundtrip("010F000080060000000103000080010000000500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000F03F0000000000000000000000000000F03F000000000000F03F0000000000000000000000000000F03F0000000000000000000000000000000000000000000000000000000000000000010300008001000000050000000000000000000000000000000000000000000000000000000000000000000000000000000000F03F0000000000000000000000000000F03F000000000000F03F0000000000000000000000000000F03F0000000000000000000000000000000000000000000000000000000000000000000000000000000001030000800100000005000000000000000000000000000000000000000000000000000000000000000000F03F00000000000000000000000000000000000000000000F03F0000000000000000000000000000F03F00000000000000000000000000000000000000000000F03F00000000000000000000000000000000000000000000000001030000800100000005000000000000000000F03F000000000000F03F0000000000000000000000000000F03F000000000000F03F000000000000F03F000000000000F03F0000000000000000000000000000F03F000000000000F03F00000000000000000000000000000000000000000000F03F000000000000F03F0000000000000000010300008001000000050000000000000000000000000000000000F03F00000000000000000000000000000000000000000000F03F000000000000F03F000000000000F03F000000000000F03F000000000000F03F000000000000F03F000000000000F03F00000000000000000000000000000000000000000000F03F00000000000000000103000080010000000500000000000000000000000000000000000000000000000000F03F000000000000F03F0000000000000000000000000000F03F000000000000F03F000000000000F03F000000000000F03F0000000000000000000000000000F03F000000000000F03F00000000000000000000000000000000000000000000F03F", true, None));

        // SELECT 'TIN(((0 0 0,0 0 1,0 1 0,0 0 0)),((0 0 0,0 1 0,1 1 0,0 0 0)))'::geometry
        assert!(ewkb_roundtrip("0110000080020000000111000080010000000400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000F03F0000000000000000000000000000F03F0000000000000000000000000000000000000000000000000000000000000000011100008001000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000F03F0000000000000000000000000000F03F000000000000F03F0000000000000000000000000000000000000000000000000000000000000000", true, None));

        // SELECT 'TRIANGLE((0 0,0 9,9 0,0 0))'::geometry
        assert!(ewkb_roundtrip("0111000000010000000400000000000000000000000000000000000000000000000000000000000000000022400000000000002240000000000000000000000000000000000000000000000000", false, None));
    }
}
