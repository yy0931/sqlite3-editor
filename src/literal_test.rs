mod test_encode {
    use crate::literal::{Blob, Literal};

    #[test]
    fn test_string() {
        assert_eq!(
            rmp_serde::to_vec(&Literal::String("a".to_owned())).unwrap(),
            vec![0xa1, 0x61] // 0xa1 = fixstr len=1
        );
    }

    #[test]
    fn test_blob() {
        assert_eq!(
            rmp_serde::to_vec(&Literal::Blob(Blob(vec![0xff, 0xef, 0xdf]))).unwrap(),
            vec![0xc4, 0x03, 0xff, 0xef, 0xdf] // 0xc4 0x03 = bin 8 len=3
        );
    }
}

mod test_decode {
    use crate::literal::{Blob, Literal};

    #[test]
    fn test_string() {
        assert_eq!(
            rmp_serde::from_slice::<Literal>(&[0xa1, 0x61]).unwrap(), // 0xa1 = fixstr len=1
            Literal::String("a".to_owned())
        );
    }

    #[test]
    fn test_blob() {
        assert_eq!(
            rmp_serde::from_slice::<Literal>(&[0xc4, 0x03, 0xff, 0xef, 0xdf]).unwrap(), // 0xc4 0x03 = bin 8 len=3
            Literal::Blob(Blob(vec![0xff, 0xef, 0xdf]))
        );
    }

    #[test]
    fn test_uint64() {
        assert_eq!(
            rmp_serde::from_slice::<Literal>(&[0xcf, 0x0, 0x2b, 0xdc, 0x54, 0x5d, 0x6b, 0x4b, 0x87]).unwrap(),
            Literal::I64(12345678901234567)
        );
    }

    #[test]
    fn test_int64() {
        assert_eq!(
            rmp_serde::from_slice::<Literal>(&[0xd3, 0xff, 0xd4, 0x23, 0xab, 0xa2, 0x94, 0xb4, 0x79]).unwrap(),
            Literal::I64(-12345678901234567)
        );
    }

    #[test]
    fn test_float64() {
        assert_eq!(
            rmp_serde::from_slice::<Literal>(&[0xcb, 0xc0, 0x5e, 0xdd, 0x3b, 0xe2, 0x2e, 0x5d, 0xe1]).unwrap(),
            Literal::F64(-123.45678)
        );
    }
}

mod test_from_primitive {
    use crate::literal::{Blob, Literal};

    #[test]
    fn test_from_primitive() {
        assert_eq!(Into::<Literal>::into(0), Literal::I64(0));
        assert_eq!(Into::<Literal>::into(1.2), Literal::F64(1.2));
        assert_eq!(Into::<Literal>::into(false), Literal::Bool(false));
        assert_eq!(Into::<Literal>::into("a".to_owned()), Literal::String("a".to_owned()));
        assert_eq!(Into::<Literal>::into("a"), Literal::String("a".to_owned()));
        assert_eq!(Into::<Literal>::into(vec![1, 2]), Literal::Blob(Blob(vec![1, 2])));
        assert_eq!(Into::<Literal>::into(()), Literal::Nil);
    }
}
