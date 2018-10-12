use std::error::Error as StdError;
use std::io::prelude::*;

use diesel::deserialize::{self, FromSql};
use diesel::pg::data_types::PgNumeric;
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Numeric;

/// This is a wrapper for monetary amounts in blockchain.
/// You have to be careful that it has a limited amount of 38 significant digits
/// So make sure that total monetary supply of a coin (in satoshis, wei, etc) does not exceed that.
/// It has json and postgres serialization / deserialization implemented.
/// Numeric type from postgres has bigger precision, so you need to impose contraint
/// that your db contains only limited precision numbers, i.e. no floating point and limited by u128 values.
///
/// As a monetary amount it only implements checked_add and checked_sub
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, FromSqlRow, AsExpression, Default, PartialOrd)]
#[sql_type = "Numeric"]
pub struct Amount(u128);

impl Amount {
    ///Make addition, return None on overflow
    pub fn checked_add(&self, other: Amount) -> Option<Self> {
        self.0.checked_add(other.0).map(Amount)
    }

    /// Make saubtraction, return None on overflow
    pub fn checked_sub(&self, other: Amount) -> Option<Self> {
        self.0.checked_sub(other.0).map(Amount)
    }

    pub fn new(v: u128) -> Self {
        Amount(v)
    }
}

impl<'a> From<&'a Amount> for PgNumeric {
    fn from(amount: &'a Amount) -> Self {
        u128_to_pg_decimal(amount.0)
    }
}

impl From<Amount> for PgNumeric {
    fn from(amount: Amount) -> Self {
        (&amount).into()
    }
}

impl ToSql<Numeric, Pg> for Amount {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        let numeric = PgNumeric::from(self);
        ToSql::<Numeric, Pg>::to_sql(&numeric, out)
    }
}

impl FromSql<Numeric, Pg> for Amount {
    fn from_sql(numeric: Option<&[u8]>) -> deserialize::Result<Self> {
        let numeric = PgNumeric::from_sql(numeric)?;
        pg_decimal_to_u128(&numeric).map(Amount)
    }
}

// Iterator over the digits of a big uint in base 10k.
// The digits will be returned in little endian order.
struct ToBase10000(Option<u128>);

impl Iterator for ToBase10000 {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.take().map(|v| {
            let rem = v % 10_000u128;
            let div = v / 10_000u128;
            if div != 0 {
                self.0 = Some(div);
            }
            rem as i16
        })
    }
}

// to check binary posgres numeric representation
// psql -U postgres -d challenge -c 'COPY ( SELECT 1000000001000000000000000000 ) TO STDOUT WITH ( FORMAT BINARY );' |   od --skip-bytes=25 -h --endian big
// bytes are: digits_count, weight, sign, scale, digit1, digit2, ..., last 2 bytes are trash

fn pg_decimal_to_u128(numeric: &PgNumeric) -> deserialize::Result<u128> {
    let (weight, scale, digits) = match *numeric {
        PgNumeric::Positive { weight, scale, ref digits } => (weight, scale, digits),
        PgNumeric::Negative { .. } => return Err(Box::from(format!("Negative is not supported in u128: {:#?}", numeric))),
        PgNumeric::NaN => return Err(Box::from(format!("NaN is not supported in u128: {:#?}", numeric))),
    };

    if scale != 0 {
        return Err(Box::from(format!("Nonzero scale is not supported in u128: {:#?}", numeric)));
    }

    if weight < 0 {
        return Err(Box::from(format!("Negative weight is not supported in u128: {:#?}", numeric)));
    }

    let mut result = 0u128;
    for digit in digits {
        result = result
            .checked_mul(10_000u128)
            .and_then(|res| res.checked_add(*digit as u128))
            .ok_or(Box::from(format!("Overflow in Pgnumeric to u128 (digits phase): {:#?}", numeric)) as Box<StdError + Send + Sync>)?;
    }

    let correction_exp = 4 * ((i32::from(weight)) - (digits.len() as i32) + 1);
    if correction_exp < 0 {
        return Err(Box::from(format!(
            "Negative correction exp is not supported in u128: {:#?}",
            numeric
        )));
    }
    // Todo - checked by using iteration;
    let pow = 10u128.pow(correction_exp as u32);
    let result = result
        .checked_mul(pow)
        .ok_or(Box::from(format!("Overflow in Pgnumeric to u128 (correction phase): {:#?}", numeric)) as Box<StdError + Send + Sync>)?;
    Ok(result)
}

fn u128_to_pg_decimal(value: u128) -> PgNumeric {
    let digits = ToBase10000(Some(value)).collect::<Vec<_>>();
    let weight = digits.len() as i16 - 1;
    let mut digits: Vec<i16> = digits.into_iter().skip_while(|digit| *digit == 0).collect();
    digits.reverse();

    PgNumeric::Positive { digits, scale: 0, weight }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // This thing converts binary postgres representation to PgNumeric
    // All test cases are generated using postgres command
    // psql -U postgres -d <your_db_name> -c 'COPY ( SELECT CAST (34534 AS NUMERIC) ) TO STDOUT WITH ( FORMAT BINARY );' |   od --skip-bytes=25 -h --endian big
    // bytes are: digits_count, weight, sign, scale, digit1, digit2, ..., last 2 bytes are trash and always equal ffff
    struct PgBinary(String);

    impl Into<PgNumeric> for PgBinary {
        fn into(self) -> PgNumeric {
            let bytes: Vec<i64> = self.0.split(" ").map(|x| i64::from_str_radix(x, 16).unwrap()).collect();
            let weight = bytes[1] as i16;
            let sign = bytes[2];
            let scale = bytes[3] as i16;
            let digits: Vec<i16> = bytes[4..].iter().map(|x| *x as i16).collect();

            match sign {
                0 => PgNumeric::Positive {
                    weight,
                    scale: scale as u16,
                    digits,
                },
                0x4000 => PgNumeric::Negative {
                    weight,
                    scale: scale as u16,
                    digits,
                },
                _ => PgNumeric::NaN,
            }
        }
    }

    #[test]
    fn test_pg_numeric_happy_conversions() {
        let cases = [
            ("0003 0006 0000 0000 03e8 0000 03e8", 1000000010000000000000000000u128),
            (
                "0009 0008 0000 0000 0003 1571 0005 0000 03e8 1103 1a94 0003 1296",
                354890005000010004355680400034758u128,
            ),
            ("0000 0000 0000 0000", 0u128),
            ("0001 0000 0000 0000 0001", 1u128),
            ("0001 0000 0000 0000 0002", 2u128),
            ("0001 0000 0000 0000 000a", 10u128),
            ("0001 0000 0000 0000 270f", 9999u128),
            ("0001 0001 0000 0000 0001", 10000u128),
            ("0002 0001 0000 0000 0001 0001", 10001u128),
            ("0002 0001 0000 0000 0001 0457", 11111u128),
            ("0002 0001 0000 0000 15b3 15b3", 55555555u128),
            ("0002 0001 0000 0000 270f 270f", 99999999u128),
            ("0003 0004 0000 0000 04d5 268f 095e", 12379871239800000000u128),
            (
                "000a 0009 0000 0000 0154 0b07 1a24 03aa 121a 18c1 11ff 10dd 1aa5 05ae",
                340282366920938463463374607431768211454u128,
            ),
            (
                "000a 0009 0000 0000 0154 0b07 1a24 03aa 121a 18c1 11ff 10dd 1aa5 05af",
                // u128 max value
                340282366920938463463374607431768211455u128,
            ),
        ];
        for case in cases.into_iter() {
            let (binary, number) = case.clone();
            let binary: PgBinary = PgBinary(binary.to_string());
            let pg_num: PgNumeric = binary.into();
            assert_eq!(pg_num, u128_to_pg_decimal(number), "u128 -> PgDecimal");
            assert_eq!(number, pg_decimal_to_u128(&pg_num).unwrap(), "PgDecimal -> u128");
        }
    }

    #[test]
    fn test_pg_numeric_error_conversions() {
        let error_cases = [
            // Nan
            "0000 0000 C000 0000",
            // -1
            "0001 0000 4000 0000 0001",
            // -10_000
            "0001 0001 4000 0000 0001",
            // 0.1
            "0001 ffff 0000 0001 03e8",
            // 0.00001
            "0001 fffe 0000 0005 03e8",
            // 1.1
            "0002 0000 0000 0001 0001 03e8",
            // 10000.00001
            "0004 0001 0000 0005 0001 0000 0000 03e8",
            // u128::max_value + 1
            "000a 0009 0000 0000 0154 0b07 1a24 03aa 121a 18c1 11ff 10dd 1aa5 05b0",
            // u128::max_value.1
            "000b 0009 0000 0001 0154 0b07 1a24 03aa 121a 18c1 11ff 10dd 1aa5 05af 03e8",
            // -u128::max_value
            "000a 0009 4000 0000 0154 0b07 1a24 03aa 121a 18c1 11ff 10dd 1aa5 05af",
            // i128::min_value
            "000a 0009 4000 0000 00aa 0583 209a 01d5 090d 0c60 1c87 1bf6 20da 1660",
            // i128::min_value - 1
            "000a 0009 4000 0000 00aa 0583 209a 01d5 090d 0c60 1c87 1bf6 20da 1661",
        ];
        for case in error_cases.into_iter() {
            let binary: PgBinary = PgBinary(case.to_string());
            let pg_num: PgNumeric = binary.into();
            assert_eq!(pg_decimal_to_u128(&pg_num).is_err(), true, "Case: {}", case);
        }
    }

    #[test]
    fn test_serde_conversions() {
        let cases = [
            ("1000000010000000000000000000", 1000000010000000000000000000u128),
            ("354890005000010004355680400034758", 354890005000010004355680400034758u128),
            ("0", 0u128),
            ("1", 1u128),
            ("2", 2u128),
            ("10", 10u128),
            ("9999", 9999u128),
            ("10000", 10000u128),
            ("10001", 10001u128),
            ("11111", 11111u128),
            ("55555555", 55555555u128),
            ("99999999", 99999999u128),
            ("12379871239800000000", 12379871239800000000u128),
            (
                // u128 max value - 1
                "340282366920938463463374607431768211454",
                340282366920938463463374607431768211454u128,
            ),
            (
                "340282366920938463463374607431768211455",
                // u128 max value
                340282366920938463463374607431768211455u128,
            ),
        ];
        for case in cases.into_iter() {
            let (string, number) = case.clone();
            let parsed: Amount = serde_json::from_str(string).unwrap();
            assert_eq!(parsed, Amount(number));
        }
    }

    #[test]
    fn test_serde_error_conversions() {
        let error_cases = [
            "-1",
            "-10000",
            "0.1",
            "0.00001",
            "1.1",
            "10000.00001",
            // u128::max_value + 1
            "340282366920938463463374607431768211456",
            // u128::max_value.1
            "340282366920938463463374607431768211455.1",
            // -u128::max_value
            "-340282366920938463463374607431768211455",
            // i128::min_value
            "-170141183460469231731687303715884105728",
            // i128::min_value - 1
            "-170141183460469231731687303715884105729",
        ];
        for case in error_cases.into_iter() {
            let parsed: Result<Amount, _> = serde_json::from_str(case);
            assert_eq!(parsed.is_err(), true, "Case: {}", case);
        }
    }

    #[test]
    fn test_checked_ops() {
        assert_eq!(Amount(5).checked_add(Amount(8)), Some(Amount(13)));
        assert_eq!(Amount(u128::max_value()).checked_add(Amount(1)), None);
        assert_eq!(Amount(u128::max_value()).checked_sub(Amount(u128::max_value())), Some(Amount(0)));
        assert_eq!(Amount(13).checked_sub(Amount(11)), Some(Amount(2)));
        assert_eq!(Amount(8).checked_sub(Amount(11)), None);
    }

}
