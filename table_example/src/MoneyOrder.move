module 0x42::DummyCoin {
    struct Coin has store { value: u64 }

    public fun zero(): Coin {
        Coin { value: 0 }
    }

    public fun join(c1: Coin, c2: Coin): Coin {
        let Coin { value: v1 } = c1;
        let Coin { value: v2 } = c2;
        Coin { value: v1 + v2 }
    }
}
module 0x42::DummyTimestamp {
    public fun now_microseconds(): u64 { 0 }
}

module 0x42::MoneyOrders {
    use Std::Table::{Self, Table};
    use 0x42::DummyCoin::Coin;
    use 0x42::DummyTimestamp::now_microseconds;

    const OWNER: address = @0x42;

    struct MoneyOrders has key {
        table: Table<vector<u8>, MoneyOrder>,
    }

    struct MoneyOrder has store {
        coin: Coin,
        timeout: u64,
    }

    public(script) fun init(s: &signer) {
        assert(Std::Signer::address_of(s) == OWNER, 42);
        let mos =  MoneyOrders {
            table: Table::create(s),
        };
        move_to<MoneyOrders>(s, mos)
    }

    public fun insert(key: vector<u8>, coin: Coin, timeout: u64) acquires MoneyOrders {
        let mos = borrow_global_mut<MoneyOrders>(OWNER);
        let mo = MoneyOrder { coin, timeout };
        Table::insert_or_abort(&mut mos.table, key, mo)
    }

    public fun redeem(key: vector<u8>): Coin acquires MoneyOrders {
        let mos = borrow_global_mut<MoneyOrders>(OWNER);
        let MoneyOrder { coin, timeout } = Table::remove_or_abort(&mut mos.table, &key);
        assert(timeout > now_microseconds(), 42);
        coin
    }

}

module 0x42::MoneyOrdersWithCleanup {
    use Std::LinkedTable::{Self, LinkedTable};
    use Std::Option;
    use 0x42::DummyCoin::{Self as Coin, Coin};
    use 0x42::DummyTimestamp::now_microseconds;

    const OWNER: address = @0x42;

    struct MoneyOrders has key {
        table: LinkedTable<vector<u8>, MoneyOrder>,
    }

    struct MoneyOrder has store {
        coin: Coin,
        timeout: u64,
    }

    public(script) fun init(s: &signer) {
        assert(Std::Signer::address_of(s) == OWNER, 42);
        let mos =  MoneyOrders {
            table: LinkedTable::create(s),
        };
        move_to<MoneyOrders>(s, mos)
    }

    public fun insert(key: vector<u8>, coin: Coin, timeout: u64) acquires MoneyOrders {
        let mos = borrow_global_mut<MoneyOrders>(OWNER);

        LinkedTable::insert_or_abort(&mut mos.table, key, MoneyOrder { coin, timeout })
    }

    public fun redeem(key: vector<u8>): Coin acquires MoneyOrders {
        let mos = borrow_global_mut<MoneyOrders>(OWNER);
        let (MoneyOrder { coin, timeout }, _, _) =
            LinkedTable::remove_or_abort(&mut mos.table, &key);
        assert(timeout > now_microseconds(), 42);

        coin
    }

    public fun super_slow_cleanup(s: &signer): Coin acquires MoneyOrders  {
        assert(Std::Signer::address_of(s) == OWNER, 42);
        let mos = borrow_global_mut<MoneyOrders>(OWNER);
        let result = Coin::zero();
        let cur_opt = LinkedTable::head(&mos.table);
        while (Option::is_some(&cur_opt)) {
            let cur = Option::borrow(&cur_opt);
            if (LinkedTable::borrow(&mos.table, cur).timeout > now_microseconds()) {
                cur_opt = LinkedTable::next(&mos.table, cur);
                continue
            };

            let (MoneyOrder { coin, timeout: _ }, _, next_opt) =
                LinkedTable::remove_or_abort(&mut mos.table, cur);
            result = Coin::join(result, coin);
            cur_opt = next_opt;
        };
        result
    }

}
