# Change Log

## Unreleased

### Changed

*   BREAKING CHANGE: The `ParamSetId` type has been removed and replaced with separate `PicParamSetId` and
    `SeqParamSetId` types, since the allowed range of values needs to be different in these two usages.
*   BREAKING CHANGE: The `rbsp::ByteReader::new` constructor has been removed in favor of more explicit
    `ByteReader::skipping_h264_header`, alongside the new `ByteReader::without_skip` and `ByteReader::skipping_bytes`
    that are suitable for other situations or parsing H.265 streams with two-byte NAL headers.
*   BREAKING CHANGE: the `rbsp::BitReaderError::ReadError` has been removed; methods consistently return
    the variant `rbsp::BitReaderError::ReadErrorFor` which additionally supplies the field name.
*   BREAKING CHANGE: some methods in `rbsp::BitRead` have been renamed to match the `bitstream-io` conventions.

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
