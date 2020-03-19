use crate::nal::sei::HeaderType;
use crate::Context;
use crate::nal::sei::SeiCompletePayloadReader;

#[derive(Debug)]
pub enum ItuTT35Error {
    NotEnoughData { expected: usize, actual: usize }
}

#[derive(Debug, PartialEq)]
pub enum ItuTT35 {
    Japan,
    Albania,
    Algeria,
    AmericanSamoa,
    GermanyFederalRepublicOf(u8),
    Anguilla,
    AntiguaandBarbuda,
    Argentina,
    AscensionseeSHelena,
    Australia,
    Austria,
    Bahamas,
    Bahrain,
    Bangladesh,
    Barbados,
    Belgium,
    Belize,
    BeninRepublicOf,
    Bermudas,
    BhutanKingdomOf,
    Bolivia,
    Botswana,
    Brazil,
    BritishAntarcticTerritory,
    BritishIndianOceanTerritory,
    BritishVirginIslands,
    BruneiDarussalam,
    Bulgaria,
    MyanmarUnionOf,
    Burundi,
    Byelorussia,
    Cameroon,
    Canada,
    CapeVerde,
    CaymanIslands,
    CentralAfricanRepublic,
    Chad,
    Chile,
    China,
    Colombia,
    Comoros,
    Congo,
    CookIslands,
    CostaRica,
    Cuba,
    Cyprus,
    CzechandSlovakFederalRepublic,
    Cambodia,
    DemocraticPeoplesRepublicOfKorea,
    Denmark,
    Djibouti,
    DominicanRepublic,
    Dominica,
    Ecuador,
    Egypt,
    ElSalvador,
    EquatorialGuinea,
    Ethiopia,
    FalklandIslands,
    Fiji,
    Finland,
    France,
    FrenchPolynesia,
    FrenchSouthernAndAntarcticLands,
    Gabon,
    Gambia,
    Angola,
    Ghana,
    Gibraltar,
    Greece,
    Grenada,
    Guam,
    Guatemala,
    Guernsey,
    Guinea,
    GuineaBissau,
    Guayana,
    Haiti,
    Honduras,
    Hongkong,
    HungaryRepublicOf,
    Iceland,
    India,
    Indonesia,
    IranIslamicRepublicOf,
    Iraq,
    Ireland,
    Israel,
    Italy,
    CotedIvoire,
    Jamaica,
    Afghanistan,
    Jersey,
    Jordan,
    Kenya,
    Kiribati,
    KoreaRepublicOf,
    Kuwait,
    LaoPeoplesDemocraticRepublic,
    Lebanon,
    Lesotho,
    Liberia,
    Libya,
    Liechtenstein,
    Luxembourg,
    Macau,
    Madagascar,
    Malaysia,
    Malawi,
    Maldives,
    Mali,
    Malta,
    Mauritania,
    Mauritius,
    Mexico,
    Monaco,
    Mongolia,
    Montserrat,
    Morocco,
    Mozambique,
    Nauru,
    Nepal,
    Netherlands,
    NetherlandsAntilles,
    NewCaledonia,
    NewZealand,
    Nicaragua,
    Niger,
    Nigeria,
    Norway,
    Oman,
    Pakistan,
    Panama,
    PapuaNewGuinea,
    Paraguay,
    Peru,
    Philippines,
    PolandRepublicOf,
    Portugal,
    PuertoRico,
    Qatar,
    Romania,
    Rwanda,
    SaintKittsAndNevis,
    SaintCroix,
    SaintHelenaAndAscension,
    SaintLucia,
    SanMarino,
    SaintThomas,
    SaoTomeAndPrincipe,
    SaintVincentAndTheGrenadines,
    SaudiArabia,
    Senegal,
    Seychelles,
    SierraLeone,
    Singapore,
    SolomonIslands,
    Somalia,
    SouthAfrica,
    Spain,
    SriLanka,
    Sudan,
    Suriname,
    Swaziland,
    Sweden,
    Switzerland,
    Syria,
    Tanzania,
    Thailand,
    Togo,
    Tonga,
    TrinidadAndTobago,
    Tunisia,
    Turkey,
    TurksAndCaicosIslands,
    Tuvalu,
    Uganda,
    Ukraine,
    UnitedArabEmirates,
    UnitedKingdom,
    UnitedStates,
    BurkinaFaso,
    Uruguay,
    USSR,
    Vanuatu,
    VaticanCityState,
    Venezuela,
    VietNam,
    WallisAndFutuna,
    WesternSamoa,
    YemenRepublicOf(u8),
    Yugoslavia,
    Zaire,
    Zambia,
    Zimbabwe,
    Unknown(u8),
    Extended(u8),
}
impl ItuTT35 {
    fn read(buf: &[u8]) -> Result<(ItuTT35, &[u8]), ItuTT35Error> {
        if buf.is_empty() {
            return Err(ItuTT35Error::NotEnoughData { expected: 1, actual: 0 });
        }
        let itu_t_t35_country_code = buf[0];
        Ok(match itu_t_t35_country_code {
            0b0000_0000 => (ItuTT35::Japan, &buf[1..]),
            0b0000_0001 => (ItuTT35::Albania, &buf[1..]),
            0b0000_0010 => (ItuTT35::Algeria, &buf[1..]),
            0b0000_0011 => (ItuTT35::AmericanSamoa, &buf[1..]),
            0b0000_0100 => (ItuTT35::GermanyFederalRepublicOf(itu_t_t35_country_code), &buf[1..]),
            0b0000_0101 => (ItuTT35::Anguilla, &buf[1..]),
            0b0000_0110 => (ItuTT35::AntiguaandBarbuda, &buf[1..]),
            0b0000_0111 => (ItuTT35::Argentina, &buf[1..]),
            0b0000_1000 => (ItuTT35::AscensionseeSHelena, &buf[1..]),
            0b0000_1001 => (ItuTT35::Australia, &buf[1..]),
            0b0000_1010 => (ItuTT35::Austria, &buf[1..]),
            0b0000_1011 => (ItuTT35::Bahamas, &buf[1..]),
            0b0000_1100 => (ItuTT35::Bahrain, &buf[1..]),
            0b0000_1101 => (ItuTT35::Bangladesh, &buf[1..]),
            0b0000_1110 => (ItuTT35::Barbados, &buf[1..]),
            0b0000_1111 => (ItuTT35::Belgium, &buf[1..]),
            0b0001_0000 => (ItuTT35::Belize, &buf[1..]),
            0b0001_0001 => (ItuTT35::BeninRepublicOf, &buf[1..]),
            0b0001_0010 => (ItuTT35::Bermudas, &buf[1..]),
            0b0001_0011 => (ItuTT35::BhutanKingdomOf, &buf[1..]),
            0b0001_0100 => (ItuTT35::Bolivia, &buf[1..]),
            0b0001_0101 => (ItuTT35::Botswana, &buf[1..]),
            0b0001_0110 => (ItuTT35::Brazil, &buf[1..]),
            0b0001_0111 => (ItuTT35::BritishAntarcticTerritory, &buf[1..]),
            0b0001_1000 => (ItuTT35::BritishIndianOceanTerritory, &buf[1..]),
            0b0001_1001 => (ItuTT35::BritishVirginIslands, &buf[1..]),
            0b0001_1010 => (ItuTT35::BruneiDarussalam, &buf[1..]),
            0b0001_1011 => (ItuTT35::Bulgaria, &buf[1..]),
            0b0001_1100 => (ItuTT35::MyanmarUnionOf, &buf[1..]),
            0b0001_1101 => (ItuTT35::Burundi, &buf[1..]),
            0b0001_1110 => (ItuTT35::Byelorussia, &buf[1..]),
            0b0001_1111 => (ItuTT35::Cameroon, &buf[1..]),
            0b0010_0000 => (ItuTT35::Canada, &buf[1..]),
            0b0010_0001 => (ItuTT35::CapeVerde, &buf[1..]),
            0b0010_0010 => (ItuTT35::CaymanIslands, &buf[1..]),
            0b0010_0011 => (ItuTT35::CentralAfricanRepublic, &buf[1..]),
            0b0010_0100 => (ItuTT35::Chad, &buf[1..]),
            0b0010_0101 => (ItuTT35::Chile, &buf[1..]),
            0b0010_0110 => (ItuTT35::China, &buf[1..]),
            0b0010_0111 => (ItuTT35::Colombia, &buf[1..]),
            0b0010_1000 => (ItuTT35::Comoros, &buf[1..]),
            0b0010_1001 => (ItuTT35::Congo, &buf[1..]),
            0b0010_1010 => (ItuTT35::CookIslands, &buf[1..]),
            0b0010_1011 => (ItuTT35::CostaRica, &buf[1..]),
            0b0010_1100 => (ItuTT35::Cuba, &buf[1..]),
            0b0010_1101 => (ItuTT35::Cyprus, &buf[1..]),
            0b0010_1110 => (ItuTT35::CzechandSlovakFederalRepublic, &buf[1..]),
            0b0010_1111 => (ItuTT35::Cambodia, &buf[1..]),
            0b0011_0000 => (ItuTT35::DemocraticPeoplesRepublicOfKorea, &buf[1..]),
            0b0011_0001 => (ItuTT35::Denmark, &buf[1..]),
            0b0011_0010 => (ItuTT35::Djibouti, &buf[1..]),
            0b0011_0011 => (ItuTT35::DominicanRepublic, &buf[1..]),
            0b0011_0100 => (ItuTT35::Dominica, &buf[1..]),
            0b0011_0101 => (ItuTT35::Ecuador, &buf[1..]),
            0b0011_0110 => (ItuTT35::Egypt, &buf[1..]),
            0b0011_0111 => (ItuTT35::ElSalvador, &buf[1..]),
            0b0011_1000 => (ItuTT35::EquatorialGuinea, &buf[1..]),
            0b0011_1001 => (ItuTT35::Ethiopia, &buf[1..]),
            0b0011_1010 => (ItuTT35::FalklandIslands, &buf[1..]),
            0b0011_1011 => (ItuTT35::Fiji, &buf[1..]),
            0b0011_1100 => (ItuTT35::Finland, &buf[1..]),
            0b0011_1101 => (ItuTT35::France, &buf[1..]),
            0b0011_1110 => (ItuTT35::FrenchPolynesia, &buf[1..]),
            0b0011_1111 => (ItuTT35::FrenchSouthernAndAntarcticLands, &buf[1..]),
            0b0100_0000 => (ItuTT35::Gabon, &buf[1..]),
            0b0100_0001 => (ItuTT35::Gambia, &buf[1..]),
            0b0100_0010 => (ItuTT35::GermanyFederalRepublicOf(itu_t_t35_country_code), &buf[1..]),
            0b0100_0011 => (ItuTT35::Angola, &buf[1..]),
            0b0100_0100 => (ItuTT35::Ghana, &buf[1..]),
            0b0100_0101 => (ItuTT35::Gibraltar, &buf[1..]),
            0b0100_0110 => (ItuTT35::Greece, &buf[1..]),
            0b0100_0111 => (ItuTT35::Grenada, &buf[1..]),
            0b0100_1000 => (ItuTT35::Guam, &buf[1..]),
            0b0100_1001 => (ItuTT35::Guatemala, &buf[1..]),
            0b0100_1010 => (ItuTT35::Guernsey, &buf[1..]),
            0b0100_1011 => (ItuTT35::Guinea, &buf[1..]),
            0b0100_1100 => (ItuTT35::GuineaBissau, &buf[1..]),
            0b0100_1101 => (ItuTT35::Guayana, &buf[1..]),
            0b0100_1110 => (ItuTT35::Haiti, &buf[1..]),
            0b0100_1111 => (ItuTT35::Honduras, &buf[1..]),
            0b0101_0000 => (ItuTT35::Hongkong, &buf[1..]),
            0b0101_0001 => (ItuTT35::HungaryRepublicOf, &buf[1..]),
            0b0101_0010 => (ItuTT35::Iceland, &buf[1..]),
            0b0101_0011 => (ItuTT35::India, &buf[1..]),
            0b0101_0100 => (ItuTT35::Indonesia, &buf[1..]),
            0b0101_0101 => (ItuTT35::IranIslamicRepublicOf, &buf[1..]),
            0b0101_0110 => (ItuTT35::Iraq, &buf[1..]),
            0b0101_0111 => (ItuTT35::Ireland, &buf[1..]),
            0b0101_1000 => (ItuTT35::Israel, &buf[1..]),
            0b0101_1001 => (ItuTT35::Italy, &buf[1..]),
            0b0101_1010 => (ItuTT35::CotedIvoire, &buf[1..]),
            0b0101_1011 => (ItuTT35::Jamaica, &buf[1..]),
            0b0101_1100 => (ItuTT35::Afghanistan, &buf[1..]),
            0b0101_1101 => (ItuTT35::Jersey, &buf[1..]),
            0b0101_1110 => (ItuTT35::Jordan, &buf[1..]),
            0b0101_1111 => (ItuTT35::Kenya, &buf[1..]),
            0b0110_0000 => (ItuTT35::Kiribati, &buf[1..]),
            0b0110_0001 => (ItuTT35::KoreaRepublicOf, &buf[1..]),
            0b0110_0010 => (ItuTT35::Kuwait, &buf[1..]),
            0b0110_0011 => (ItuTT35::LaoPeoplesDemocraticRepublic, &buf[1..]),
            0b0110_0100 => (ItuTT35::Lebanon, &buf[1..]),
            0b0110_0101 => (ItuTT35::Lesotho, &buf[1..]),
            0b0110_0110 => (ItuTT35::Liberia, &buf[1..]),
            0b0110_0111 => (ItuTT35::Libya, &buf[1..]),
            0b0110_1000 => (ItuTT35::Liechtenstein, &buf[1..]),
            0b0110_1001 => (ItuTT35::Luxembourg, &buf[1..]),
            0b0110_1010 => (ItuTT35::Macau, &buf[1..]),
            0b0110_1011 => (ItuTT35::Madagascar, &buf[1..]),
            0b0110_1100 => (ItuTT35::Malaysia, &buf[1..]),
            0b0110_1101 => (ItuTT35::Malawi, &buf[1..]),
            0b0110_1110 => (ItuTT35::Maldives, &buf[1..]),
            0b0110_1111 => (ItuTT35::Mali, &buf[1..]),
            0b0111_0000 => (ItuTT35::Malta, &buf[1..]),
            0b1111_0001 => (ItuTT35::Mauritania, &buf[1..]),
            0b0111_0010 => (ItuTT35::Mauritius, &buf[1..]),
            0b0111_0011 => (ItuTT35::Mexico, &buf[1..]),
            0b0111_0100 => (ItuTT35::Monaco, &buf[1..]),
            0b0111_0101 => (ItuTT35::Mongolia, &buf[1..]),
            0b0111_0110 => (ItuTT35::Montserrat, &buf[1..]),
            0b0111_0111 => (ItuTT35::Morocco, &buf[1..]),
            0b0111_1000 => (ItuTT35::Mozambique, &buf[1..]),
            0b0111_1001 => (ItuTT35::Nauru, &buf[1..]),
            0b0111_1010 => (ItuTT35::Nepal, &buf[1..]),
            0b0111_1011 => (ItuTT35::Netherlands, &buf[1..]),
            0b0111_1100 => (ItuTT35::NetherlandsAntilles, &buf[1..]),
            0b0111_1101 => (ItuTT35::NewCaledonia, &buf[1..]),
            0b0111_1110 => (ItuTT35::NewZealand, &buf[1..]),
            0b0111_1111 => (ItuTT35::Nicaragua, &buf[1..]),
            0b1000_0000 => (ItuTT35::Niger, &buf[1..]),
            0b1000_0001 => (ItuTT35::Nigeria, &buf[1..]),
            0b1000_0010 => (ItuTT35::Norway, &buf[1..]),
            0b1000_0011 => (ItuTT35::Oman, &buf[1..]),
            0b1000_0100 => (ItuTT35::Pakistan, &buf[1..]),
            0b1000_0101 => (ItuTT35::Panama, &buf[1..]),
            0b1000_0110 => (ItuTT35::PapuaNewGuinea, &buf[1..]),
            0b1000_0111 => (ItuTT35::Paraguay, &buf[1..]),
            0b1000_1000 => (ItuTT35::Peru, &buf[1..]),
            0b1000_1001 => (ItuTT35::Philippines, &buf[1..]),
            0b1000_1010 => (ItuTT35::PolandRepublicOf, &buf[1..]),
            0b1000_1011 => (ItuTT35::Portugal, &buf[1..]),
            0b1000_1100 => (ItuTT35::PuertoRico, &buf[1..]),
            0b1000_1101 => (ItuTT35::Qatar, &buf[1..]),
            0b1000_1110 => (ItuTT35::Romania, &buf[1..]),
            0b1000_1111 => (ItuTT35::Rwanda, &buf[1..]),
            0b1001_0000 => (ItuTT35::SaintKittsAndNevis, &buf[1..]),
            0b1001_0001 => (ItuTT35::SaintCroix, &buf[1..]),
            0b1001_0010 => (ItuTT35::SaintHelenaAndAscension, &buf[1..]),
            0b1001_0011 => (ItuTT35::SaintLucia, &buf[1..]),
            0b1001_0100 => (ItuTT35::SanMarino, &buf[1..]),
            0b1001_0101 => (ItuTT35::SaintThomas, &buf[1..]),
            0b1001_0110 => (ItuTT35::SaoTomeAndPrincipe, &buf[1..]),
            0b1001_0111 => (ItuTT35::SaintVincentAndTheGrenadines, &buf[1..]),
            0b1001_1000 => (ItuTT35::SaudiArabia, &buf[1..]),
            0b1001_1001 => (ItuTT35::Senegal, &buf[1..]),
            0b1001_1010 => (ItuTT35::Seychelles, &buf[1..]),
            0b1001_1011 => (ItuTT35::SierraLeone, &buf[1..]),
            0b1001_1100 => (ItuTT35::Singapore, &buf[1..]),
            0b1001_1101 => (ItuTT35::SolomonIslands, &buf[1..]),
            0b1001_1110 => (ItuTT35::Somalia, &buf[1..]),
            0b1001_1111 => (ItuTT35::SouthAfrica, &buf[1..]),
            0b1010_0000 => (ItuTT35::Spain, &buf[1..]),
            0b1010_0001 => (ItuTT35::SriLanka, &buf[1..]),
            0b1010_0010 => (ItuTT35::Sudan, &buf[1..]),
            0b1010_0011 => (ItuTT35::Suriname, &buf[1..]),
            0b1010_0100 => (ItuTT35::Swaziland, &buf[1..]),
            0b1010_0101 => (ItuTT35::Sweden, &buf[1..]),
            0b1010_0110 => (ItuTT35::Switzerland, &buf[1..]),
            0b1010_0111 => (ItuTT35::Syria, &buf[1..]),
            0b1010_1000 => (ItuTT35::Tanzania, &buf[1..]),
            0b1010_1001 => (ItuTT35::Thailand, &buf[1..]),
            0b1010_1010 => (ItuTT35::Togo, &buf[1..]),
            0b1010_1011 => (ItuTT35::Tonga, &buf[1..]),
            0b1010_1100 => (ItuTT35::TrinidadAndTobago, &buf[1..]),
            0b1010_1101 => (ItuTT35::Tunisia, &buf[1..]),
            0b1010_1110 => (ItuTT35::Turkey, &buf[1..]),
            0b1010_1111 => (ItuTT35::TurksAndCaicosIslands, &buf[1..]),
            0b1011_0000 => (ItuTT35::Tuvalu, &buf[1..]),
            0b1011_0001 => (ItuTT35::Uganda, &buf[1..]),
            0b1011_0010 => (ItuTT35::Ukraine, &buf[1..]),
            0b1011_0011 => (ItuTT35::UnitedArabEmirates, &buf[1..]),
            0b1011_0100 => (ItuTT35::UnitedKingdom, &buf[1..]),
            0b1011_0101 => (ItuTT35::UnitedStates, &buf[1..]),
            0b1011_0110 => (ItuTT35::BurkinaFaso, &buf[1..]),
            0b1011_0111 => (ItuTT35::Uruguay, &buf[1..]),
            0b1011_1000 => (ItuTT35::USSR, &buf[1..]),
            0b1011_1001 => (ItuTT35::Vanuatu, &buf[1..]),
            0b1011_1010 => (ItuTT35::VaticanCityState, &buf[1..]),
            0b1011_1011 => (ItuTT35::Venezuela, &buf[1..]),
            0b1011_1100 => (ItuTT35::VietNam, &buf[1..]),
            0b1011_1101 => (ItuTT35::WallisAndFutuna, &buf[1..]),
            0b1011_1110 => (ItuTT35::WesternSamoa, &buf[1..]),
            0b1011_1111 => (ItuTT35::YemenRepublicOf(itu_t_t35_country_code), &buf[1..]),
            0b1100_0000 => (ItuTT35::YemenRepublicOf(itu_t_t35_country_code), &buf[1..]),
            0b1100_0001 => (ItuTT35::Yugoslavia, &buf[1..]),
            0b1100_0010 => (ItuTT35::Zaire, &buf[1..]),
            0b1100_0011 => (ItuTT35::Zambia, &buf[1..]),
            0b1100_0100 => (ItuTT35::Zimbabwe, &buf[1..]),
            0b1111_1111 => {
                if buf.len() < 2 {
                    return Err(ItuTT35Error::NotEnoughData { expected: 2, actual: buf.len() });
                }
                (ItuTT35::Extended(buf[1]), &buf[1..])
            },
            _ => (ItuTT35::Unknown(itu_t_t35_country_code), &buf[1..]),
        })
    }
}

pub trait Register: Default {
    type Ctx;
    fn handle(&mut self, ctx: &mut Context<Self::Ctx>, country_code: ItuTT35, payload: &[u8]);
}

pub struct UserDataRegisteredItuTT35Reader<R: Register> {
    register: R,
}
impl<R: Register> UserDataRegisteredItuTT35Reader<R>  {
    pub fn new(register: R) -> UserDataRegisteredItuTT35Reader<R> {
        UserDataRegisteredItuTT35Reader {
            register,
        }
    }
}
impl<R: Register> SeiCompletePayloadReader for UserDataRegisteredItuTT35Reader<R> {
    type Ctx = R::Ctx;

    fn header(&mut self, ctx: &mut Context<Self::Ctx>, payload_type: HeaderType, buf: &[u8]) {
        assert_eq!(payload_type, HeaderType::UserDataRegisteredItuTT35);
        match ItuTT35::read(buf) {
            Ok( (country_code, payload) ) => {
                self.register.handle(ctx, country_code, payload);
            },
            Err(e) => {
                eprintln!("Failed to read user_data_registered_itu_t_t35 header: {:?}", e);
            }
        }
    }
}

#[macro_export]
macro_rules! tt_35_switch {
    (
        $struct_name:ident<$ctx:ty> {
            $( $name:ident => $v:ty ),*,
        }
    ) => {
        #[allow(non_snake_case)]
        #[derive(Default)]
        struct $struct_name {
            $( $name: $v, )*
        }
        impl $crate::nal::sei::user_data_registered_itu_t_t35::Register for $struct_name {
            type Ctx = $ctx;

            fn handle(&mut self, ctx: &mut $crate::Context<Self::Ctx>, country_code: $crate::nal::sei::user_data_registered_itu_t_t35::ItuTT35, payload: &[u8]) {
                match country_code {
                    $(
                    $crate::nal::sei::user_data_registered_itu_t_t35::ItuTT35::$name => self.$name.handle(ctx, country_code, payload),
                    )*
                    _ => (),
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Default)]
    struct NullRegister {
        handled: bool,
    }
    impl crate::nal::sei::user_data_registered_itu_t_t35::Register for NullRegister {
        type Ctx = ();

        fn handle(&mut self, _ctx: &mut crate::Context<Self::Ctx>, country_code: crate::nal::sei::user_data_registered_itu_t_t35::ItuTT35, _payload: &[u8]) {
            assert_eq!(country_code, ItuTT35::UnitedKingdom);
            self.handled = true;
        }
    }
    #[test]
    fn macro_usage() {
        tt_35_switch!{
            TestTT35Switch<()> {
                UnitedKingdom => NullRegister,
            }
        }

        let mut sw = TestTT35Switch::default();
        let mut ctx = crate::Context::new(());
        let data = [ 0x00u8 ];
        sw.handle(&mut ctx, ItuTT35::UnitedKingdom, &data[..]);
        assert!(sw.UnitedKingdom.handled);
    }
}