-- Validator for CowSwap settlement submission.

local l = string.lower
function sighash(data)
    return string.sub(data, 1, 10)
end

function validate_message()
    print("message signature not accepted")
    return false
end

function validate_transaction(account, transaction)
    if (
        l(transaction.to) ~= "0x9008d19f58aabd9ed0d60971565aa8510560ab41" or
        sighash(transaction.data) ~= "0x13d79a0b"
    ) then
        print("not a CowSwap settlement")
        return false
    end

    return true
end

function validate_typed_data(account, typed_data)
    if (
        typed_data.domain.name ~= "Gnosis Protocol" or
        typed_data.domain.version ~= "v2" or
        l(typed_data.domain.verifyingContract) ~= "0x9008d19f58aabd9ed0d60971565aa8510560ab41" or
        typed_data.primaryType ~= "Order"
    ) then
        print("not a CowSwap order")
        return false
    end

    if l(typed_data.message.buyToken) ~= "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" then
        print("only buying WETH is allowed")
        return false
    end

    if l(typed_data.message.receiver) ~= "0x6c642cafcbd9d8383250bb25f67ae409147f78b2" then
        print("proceeds not going to the team multi-sig")
        return false
    end

    return true
end

print("loaded CowSwap validator. MOO!")
