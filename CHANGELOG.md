# Change Log

## Unreleased

### Changed

*   BREAKING CHANGE: `Profile::from_profile_idc()` now accepts `ConstraintFlags` to distinguish constrained/intra profile variants. Add `Profile` enum variants: `ConstrainedBaseline`, `ProgressiveHigh`, `ConstrainedHigh`, `High10Intra`, `High422Intra`, `High444Intra`, `ScalableConstrainedBaseline`, `ScalableConstrainedHigh`, `ScalableHighIntra`.

### Added

*   Added support for `luma_weights_l1` and `chroma_weights_l1` to `PredWeightTable`.
*   Give `has_chroma_info()` newer profile_idc values 118, 128, 134, 135, 138, 139, implementing SPS parse support for Multiview High, Stereo High, and related profiles.
*   Add `Profile::CavlcIntra444` (profile_idc 44) and `Profile::MFCHigh` (profile_idc 134) enum variants.
*   Parse AvcC extension fields (`chroma_format`, `bit_depth_luma_minus8`, `bit_depth_chroma_minus8`, SPS extension NAL units) for High profile and above.
*   Add `slice_group_change_cycle`, `slice_alpha_c0_offset_div2` and `slice_beta_offset_div2 to `SliceHeader`
*   Parse `access_unit_delimiter_rbsp()` (NAL type 9) via new `aud::AccessUnitDelimiter`.
*   Parse `seq_parameter_set_extension_rbsp()` (NAL type 13) via new `sps_extension::SeqParameterSetExtension`.
*   Parse `subset_seq_parameter_set_rbsp()` (NAL type 15) via new `subset_sps::SubsetSps`, including SVC extension (profiles 83/86) and MVC extension (profiles 118/128/134).
*   Add `Context::put_subset_seq_param_set()`, `Context::subset_sps_by_id()`, and `Context::subset_sps()` for managing subset SPS state.
*   Add `ChromaInfo::chroma_array_type()` helper.

### Fixed

*   Fix `Profile::High444.profile_idc()` returning 144 instead of the correct value 244.
*   Fix `PicTiming::read_delays` falling back to `nal_hrd_parameters` instead of `vcl_hrd_parameters`.
*   Fix `pred_weight_table` being parsed for SP slices regardless of `weighted_pred_flag`.
*   Fix `ByteReader` skipping emulation prevention byte removal for bytes beyond `max_fill` in a chunk.
*   Fix off-by-one error in `SliceGroup::read_rectangles()`
*   Fix `slice_qp_delta` validation to check the derived `SliceQPY` against `[-QpBdOffsetY, 51]` per spec, instead of only checking `slice_qp_delta > 51`.
*   Will now produce an error when parsing `max_dec_frame_buffering` values that are invalid at the given profile level

## 0.8.0 - 2025-01-28

### Changed

*   BREAKING CHANGE: The `ParamSetId` type has been removed and replaced with separate `PicParamSetId` and
    `SeqParamSetId` types, since the allowed range of values needs to be different in these two usages.
*   BREAKING CHANGE: The `rbsp::ByteReader::new` constructor has been removed in favor of more explicit
    `ByteReader::skipping_h264_header`, alongside the new `ByteReader::without_skip` and `ByteReader::skipping_bytes`
    that are suitable for other situations or parsing H.265 streams with two-byte NAL headers.
*   BREAKING CHANGE: the `rbsp::BitReaderError::ReadError` has been removed; methods consistently return
    the variant `rbsp::BitReaderError::ReadErrorFor` which additionally supplies the field name.
*   BREAKING CHANGE: some methods in `rbsp::BitRead` have been renamed to match the `bitstream-io` conventions.
*   BREAKING CHANGE: updated `rfc6381-codec` version from 0.1 to 0.2.

### Added

*   Make some fields of `SliceType` public.
*   Parsing of scaling lists.

### Fixed

*   Fix parsing of `delta_pic_order_cnt` fields in `SliceHeader`.
*   Fix parsing of `slice_group_id` fields in `SliceGroup` ([#57](https://github.com/dholroyd/h264-reader/issues/57)).
*   Fix overflow on `SliceHeader.qs_y` calculation by adding bounds checks on `pps.pic_init_qs_minus26`.

## 0.7.0 - 2023-05-30

### Changed
*   Make `PicOrderCountLsb::FieldsAbsolute` field names mirror the spec, rather than doing some calculations during
    parsing.

### Fixed
*   Fixed incorrect size calculation for `PicScalingMatrix` causing parsing errors for streams having
    `pic_scaling_matrix_present_flag=1` and `transform_8x8_mode_flag=1` in the PPS.

### Added
*   Make some `SliceHeader` fields public.

## 0.6.0 - 2022-08-08

*   BREAKING CHANGE: major simplification of the push API.
*   Annex B parser bugfixes.

## 0.5.0 - 2021-06-09

*   BREAKING CHANGE: changes to error enums; switched several
    `h264_reader::rbsp::RbspBitReader` methods to return `RbspBitReaderError`
    rather than `bitreader::BitReaderError`.
*   bug fixes, mostly found by fuzzing.
*   API additions:
    *   `h264_reader::rbsp::decode_nal`.
    *   `h264_reader::nal::sps::SeqParameterSet::rfc6381`
    *   `h264_reader::nal::sps::SeqParameterSet::pixel_dimensions`
    *   exposed fields in `h264_reader::nal::sps::TimingInfo`
    *   exposed inner u8 value of `h264_reader::nal::sps::ConstraintFlags`
*   removed `read_ue` and `read_se` from
    `h264_reader::rbsp::RbspBitReader`, in favor of `_named` variants.

## 0.4.0 (31 Mar 2020, 5ef73dc)

...
