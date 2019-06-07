import logging
from abc import ABC, abstractmethod
from typing import Dict, Any, Union, List, Optional

from .database_interface import DatabaseInterface, MongoDbInterface
from .models import Deposit, StateTransition, TransitionType, Withdraw, AccountRecord, Order, AuctionSettlement


class SnappEventListener(ABC):
    """Abstract SnappEventReceiver class."""

    def __init__(self, database_interface: Optional[DatabaseInterface] = None):
        self.database = database_interface if database_interface else MongoDbInterface()
        self.logger = logging.getLogger(__name__)

    @abstractmethod
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:
        return  # pragma: no cover


class DepositReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:
        self.save_parsed(Deposit.from_dictionary(event))

    def save_parsed(self, deposit: Deposit) -> None:
        self.database.write_deposit(deposit)


class StateTransitionReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:
        self.save_parsed(StateTransition.from_dictionary(event))

    def save_parsed(self, transition: StateTransition) -> None:
        self.__update_accounts(transition)
        logging.info("Successfully updated state and balances")

    def __update_accounts(self, transition: StateTransition) -> None:
        balances = self.database.get_account_state(transition.state_index - 1).balances.copy()
        num_tokens = self.database.get_num_tokens()
        for datum in self.__get_data_to_apply(transition):
            # Balances are stored as [b(a1, t1), b(a1, t2), ... b(a1, T), b(a2, t1), ...]
            index = num_tokens * datum.account_id + datum.token_id

            if transition.transition_type == TransitionType.Deposit:
                self.logger.info(
                    "Incrementing balance of account {} - token {} by {}".format(
                        datum.account_id,
                        datum.token_id,
                        datum.amount
                    )
                )
                balances[index] += datum.amount
            elif transition.transition_type == TransitionType.Withdraw:
                assert isinstance(datum, Withdraw)
                if balances[index] - datum.amount >= 0:
                    self.logger.info(
                        "Decreasing balance of account {} - token {} by {}".format(
                            datum.account_id,
                            datum.token_id,
                            datum.amount
                        )
                    )
                    balances[index] -= datum.amount
                    self.database.update_withdraw(datum, datum._replace(valid=True))
                else:
                    self.logger.info(
                        "Insufficient balance: account {} - token {} for amount {}".format(
                            datum.account_id,
                            datum.token_id,
                            datum.amount
                        )
                    )
            else:
                self.logger.error("Unrecognized transition type: should never happen!")

        new_account_record = AccountRecord(transition.state_index, transition.state_hash, balances)
        self.database.write_account_state(new_account_record)

    def __get_data_to_apply(self, transition: StateTransition) -> Union[List[Withdraw], List[Deposit]]:
        if transition.transition_type == TransitionType.Deposit:
            return self.database.get_deposits(transition.slot)
        elif transition.transition_type == TransitionType.Withdraw:
            return self.database.get_withdraws(transition.slot)
        else:
            raise Exception("Invalid transition type: {} ".format(transition.transition_type))


class SnappInitializationReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:

        # Verify integrity of post data
        assert event.keys() == {'stateHash', 'maxTokens', 'maxAccounts'}, "Unexpected Event Keys"
        state_hash = event['stateHash']
        assert isinstance(state_hash, str) and len(state_hash) == 64, "StateHash has unexpected value %s" % state_hash
        assert isinstance(event['maxTokens'], int), "maxTokens has unexpected value"
        assert isinstance(event['maxAccounts'], int), "maxAccounts has unexpected value"

        self.initialize_accounts(event['maxTokens'], event['maxAccounts'], state_hash)

    def initialize_accounts(self, num_tokens: int, num_accounts: int, state_hash: str) -> None:
        account_record = AccountRecord(0, state_hash, [0 for _ in range(num_tokens * num_accounts)])
        self.database.write_snapp_constants(num_tokens, num_accounts)
        self.database.write_account_state(account_record)
        logging.info("Successfully included Snapp Initialization constants and account record")


class AuctionInitializationReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:

        # Verify integrity of post data
        assert event.keys() == {'maxOrders', 'numReservedAccounts', 'ordersPerReservedAccount'}, "Unexpected Event Keys"
        assert isinstance(event['maxOrders'], int), "maxOrders has unexpected value"
        assert isinstance(event['numReservedAccounts'], int), "maxOrders has unexpected value"
        assert isinstance(event['ordersPerReservedAccount'], int), "maxOrders has unexpected value"

        self.database.write_auction_constants(
            event['maxOrders'], event['numReservedAccounts'], event['ordersPerReservedAccount']
        )
        logging.info(
            "Successfully included Snapp Auction constant(s)")


class WithdrawRequestReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:
        self.save_parsed(Withdraw.from_dictionary(event))

    def save_parsed(self, withdraw: Withdraw) -> None:
        self.database.write_withdraw(withdraw)


class OrderReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:
        self.save_parsed(Order.from_dictionary(event))

    def save_parsed(self, order: Order) -> None:
        self.database.write_order(order)


class AuctionSettlementReceiver(SnappEventListener):
    def save(self, event: Dict[str, Any], block_info: Dict[str, Any]) -> None:
        self.save_parsed(
            AuctionSettlement.from_dictionary(
                event,
                self.database.get_num_tokens(),
                self.database.get_num_orders()
            )
        )

    def save_parsed(self, settlement: AuctionSettlement) -> None:
        self.__update_accounts(settlement)

    def __update_accounts(self, settlement: AuctionSettlement) -> None:
        state = self.database.get_account_state(settlement.state_index - 1)
        balances = state.balances.copy()

        orders = self.database.get_orders(settlement.auction_id)
        num_tokens = self.database.get_num_tokens()
        solution = settlement.prices_and_volumes

        buy_amounts = solution.buy_amounts
        sell_amounts = solution.sell_amounts

        for i, order in enumerate(orders):
            buy_index = num_tokens * order.account_id + order.buy_token
            balances[buy_index] += buy_amounts[i]

            sell_index = num_tokens * order.account_id + order.sell_token
            balances[sell_index] -= sell_amounts[i]

        new_account_record = AccountRecord(settlement.state_index, settlement.state_hash, balances)
        self.database.write_account_state(new_account_record)
