<template>

  <div class="container-fluid">

    <div class="row no-gutters">
      <!-- Buffer div -->
      <div class="col-1"></div>

      <!-- Main content div -->
      <div class="col-10">


          <div v-if="hashData.address_pool_info" >
            <h3 class="detail-group">AMM Swap Info</h3>
            <div class="grid-container">

              <div><strong>RDG Address</strong></div>
              <div><HashLink :shorten="false" :data="hashData.address_pool_info.addresses['Redgold']" /></div>
              <div><strong>RDG Address Balance</strong></div>
              <div>{{ parseFloat(hashData.address_pool_info.balances['Redgold'] || 0).toFixed(8)}} RDG</div>

              <div><strong>BTC Explorer Link</strong></div>
              <a :href="btcExplorerLink">{{btcExplorerLink}}</a>
              <div><strong>BTC Address</strong></div>
              <div><HashLink :shorten="false" :data="hashData.address_pool_info.addresses['Bitcoin']" /></div>
              <div><strong>BTC Balance</strong></div>
              <div>{{ parseFloat(hashData.address_pool_info.balances['Bitcoin'] || 0).toFixed(8) }} BTC</div>

              <div><strong>ETH Explorer Link</strong></div>
              <a :href="ethExplorerLink">{{ethExplorerLink}}</a>
              <div><strong>ETH Address</strong></div>
              <div><HashLink :shorten="false" :data="hashData.address_pool_info.addresses['Ethereum']" /></div>
              <div><strong>ETH Balance</strong></div>
              <div>{{ parseFloat(hashData.address_pool_info.balances['Ethereum'] || 0).toFixed(18) }} ETH</div>

              <div><strong>Public Key (Proto)</strong></div>
              <div><TextCopy :data="hashData.address_pool_info.public_key" /></div>

              <div><strong>Public Key (Compact)</strong></div>
              <div><TextCopy :data="publicKeyCompact" /></div>

              <div><strong>Price Ask USD/RDG</strong></div>
              <div><TextCopy :data="'$' + askPriceUsdRdg" /></div>
              <div><strong>Price Bid USD/RDG</strong></div>
              <div><TextCopy :data="'$' + bidPriceUsdRdg" /></div>
              <template v-for="balancePair in hashData.address_pool_info.overall_staking_balances" :key="balancePair[0]">
                <div><strong>{{balancePair[0]}} Staked</strong></div>
                <div>{{balancePair[1]}}</div>
              </template>
<!--              these are broken ?? displaying wrong balances. -->
<!--              <template v-for="balancePair in hashData.address_pool_info.amm_staking_balances" :key="balancePair[0]">-->
<!--                <div><strong>{{balancePair[0]}} AMM Staked</strong></div>-->
<!--                <div>{{balancePair[1]}}</div>-->
<!--              </template>-->
<!--              <template v-for="balancePair in hashData.address_pool_info.portfolio_staking_balances" :key="balancePair[0]">-->
<!--                <div><strong>{{balancePair[0]}} Port Staked</strong></div>-->
<!--                <div>{{balancePair[1]}}</div>-->
<!--              </template>-->

            </div>

            <!-- <h3 class="detail-group">Bid Ask AMM Curve RDG/BTC</h3>
            <div class="grid-container">
              <Bar :data="this.computeData('Bitcoin', false)" :options="exampleOptions" class="chart-container" />
              <Bar :data="this.computeData('Bitcoin', true)" :options="exampleOptions" class="chart-container" />
            </div>

            <h3 class="detail-group">Bid Ask AMM Curve RDG/ETH</h3>
            <div class="grid-container">
              <Bar :data="this.computeData('Ethereum', false)" :options="exampleOptions" class="chart-container" />
              <Bar :data="this.computeData('Ethereum', true)" :options="exampleOptions" class="chart-container" />
            </div> -->

<!--             
            <h3 class="detail-group">Trade Calculator</h3>


            <label v-if="calculatorTransactionType === 'BUY'">
              User Pair:
              <select v-model="userPair">
                <option value="USD">USD</option>
              </select>
            </label>


            <label>
              <input type="radio" v-model="calculatorTransactionType" value="BUY" /> BUY
            </label>
            <label>
              <input type="radio" v-model="calculatorTransactionType" value="SELL" /> SELL
            </label>

            <label>
              Trade Pair:
              <select v-model="activeTradePair">
                <option value="Bitcoin" >BTC</option>
                <option value="Ethereum">ETH</option>
              </select>
            </label>


            <div v-if="calculatorTransactionType === 'BUY'">
              <label>
                {{ userPair }}:
                <input type="number" class="search-input" v-model="inputUser" />
              </label>
              <label>
                {{activeTradePair}}: {{ buyCalculatedAmount }}
              </label>
            </div>
            <div v-if="calculatorTransactionType === 'SELL'">
              <label>
                RDG:
                <input type="number" v-model="inputRDG" />
              </label>
            </div>

            <div class="horizontal-display">
              <span v-if="calculatorTransactionType === 'BUY'">You'll receive:</span>
              <TextCopy v-if="calculatorTransactionType === 'BUY'" :data="rdg_buy_amount.toFixed(8)" />
              <span v-if="calculatorTransactionType === 'BUY'">RDG</span>

              <span v-if="calculatorTransactionType === 'SELL'">You'll receive:</span>
              <TextCopy v-if="calculatorTransactionType === 'SELL'" :data="btc_sell_amount.toFixed(8)"/>
              <span v-if="calculatorTransactionType === 'SELL'">{{ activeTradePair }}</span>
            </div>
           -->


            <h3 class="detail-group">AMM Events</h3>
            <div>
              <!-- <div>Debug: {{ hashData.address_pool_info.detailed_events ? 'Events exist' : 'No events' }}</div> -->
              <!-- <div>Event count: {{ hashData.address_pool_info.detailed_events ? hashData.address_pool_info.detailed_events.length : 0 }}</div> -->
              <div v-if="hashData.address_pool_info.detailed_events">
                <div><DetailedEvent :events="hashData.address_pool_info.detailed_events" /></div>
              </div>
            </div>
          </div>

          <h3 class="detail-group">Node Metadata</h3>

          <div class="grid-container">


          <div><strong>Public Key</strong></div>
          <div><HashLink :data="hashDataInitial.public_key" :shorten="false" /></div>

          <div><strong>Host</strong></div>
          <div>{{hashDataInitial.external_address}}</div>

          <div><strong>Port</strong></div>
          <div>{{hashDataInitial.port_offset}}</div>
          <div><strong>Exe Checksum</strong></div>

          <div>{{shortenExeChecksum(hashDataInitial.executable_checksum)}}</div>

          <div><strong>XOR Distance</strong></div>
          <div>{{hashDataInitial.utxo_distance}}</div>

          <div><strong>Node Name</strong></div>
          <div>{{hashDataInitial.node_name}}</div>

          <div><strong>Peer Id</strong></div>
          <div><HashLink :data="hashDataInitial.peer_id" :shorten="false" /></div>

          <!-- TODO: Observations from this PK paginated / latest observation-->
        </div>

        <h4>Recent Observations</h4>
        <BriefObservation :data="hashDataInitial.recent_observations"/>


      </div>
      <!-- Buffer div -->
      <div class="col-1"></div>
    </div>
  </div>
</template>
<script>
import HashLink from "@/components/util/HashLink.vue";
// import CopyClipboard from "@/components/util/CopyClipboard.vue";
// import PeerNodeInfo from "@/components/hash_types/PeerNodeInfo.vue";
import {toTitleCase} from "@/utils";
import BriefObservation from "@/components/BriefObservation.vue";
import DetailedEvent from "@/components/DetailedEvent.vue";
import TextCopy from "@/components/util/TextCopy.vue";

export default {
  name: "PeerInfo",
  components: {
    BriefObservation,
    // CopyClipboard,
    HashLink,
    // PeerNodeInfo,
    DetailedEvent,
    TextCopy
  },
  mounted() {
    console.log('Detailed events:', this.hashData.address_pool_info?.detailed_events);
    if (this.hashData.address_pool_info?.detailed_events) {
      console.log('Events count:', this.hashData.address_pool_info.detailed_events.length);
      console.log('First event:', this.hashData.address_pool_info.detailed_events[0]);
    }
  },
  data: function() {
    return {
      inputUSD: null,
      inputPair: null,
      inputRDG: null,
      inputUser: null,
      buyCalculatedAmount: null,
      rdg_buy_amount: 0.0,
      btc_sell_amount: 0.0,
      updatingValue: false,
      lastEdited: null,  // Will hold either 'USD' or 'BTC'
      calculatorTransactionType: 'BUY', // Default value is set to 'BUY'
      // ... other data properties ...
      transactionType: 'all',
      currentPage: 1,
      perPage: 25,
      activeTradePair: 'Bitcoin',
      userPair: 'USD',
      hashData: this.hashDataInitial,
      exampleBidAskData: {
        labels: ['January', 'February', 'March', 'April', 'May', 'June', 'July', "", "", "", ""],
        datasets: [
          {
            label: 'Data One',
            backgroundColor: '#f87979',
            data: [40, 39, 10, 40, 39, 80, 40, 40, 39, 10, 40, 39, 80, 40]
          }
        ]
      },
      ask: {
        labels: ['January', 'February', 'March', 'April', 'May', 'June', 'July', "", "", "", ""],
        datasets: [
          {
            label: 'Data One',
            backgroundColor: '#f87979',
            data: [40, 39, 10, 40, 39, 80, 40, 40, 39, 10, 40, 39, 80, 40]
          }
        ]
      },
    }
  },
  watch: {
    'hashData.address_pool_info.detailed_events': {
      handler(newEvents) {
        console.log('Events updated:', newEvents);
        if (newEvents) {
          console.log('New events count:', newEvents.length);
          console.log('First new event:', newEvents[0]);
        }
      },
      deep: true
    },

    inputUser(newUserValue) {
      let floatUserValue = parseFloat(newUserValue);
      if (this.calculatorTransactionType === 'BUY') {
        // Convert USD input to equivalent amount in selected trading pair
        if (this.userPair === "USD") {
          if (this.activeTradePair === "Bitcoin") {
            this.buyCalculatedAmount = floatUserValue / this.usdBtcRate;
          }
          if (this.activeTradePair === "Ethereum") {
            this.buyCalculatedAmount = floatUserValue / this.usdEthRate;
          }
          // Calculate RDG amount based on $100 fixed price
          this.rdg_buy_amount = floatUserValue / 100;
          this.inputPair = this.buyCalculatedAmount;
        }
      }
    },

    inputRDG(newRDGValue) {
      let floatRDGValue = parseFloat(newRDGValue);
      if (!(isNaN(floatRDGValue))) {
        // Calculate trade pair amount based on fixed RDG price of $100
        let rdgValueInUSD = floatRDGValue * 100;
        
        if (this.activeTradePair === "Bitcoin") {
          this.btc_sell_amount = rdgValueInUSD / this.usdBtcRate;
        } else if (this.activeTradePair === "Ethereum") {
          this.btc_sell_amount = rdgValueInUSD / this.usdEthRate;
        }
      }
    },
    inputPair(newBTCValue) {
      let floatBtcValue = parseFloat(newBTCValue);

      if (this.hashData.address_pool_info != null && !(isNaN(floatBtcValue))) {
        let asks = this.hashData.address_pool_info.asks[this.activeTradePair];
        let total_btc = floatBtcValue;
        let total_fulfilled = 0;
        for (let i = 0; i < asks.length; i++) {
          let ask = asks[i];
          let p = ask.price // RDG / BTC now
          let v = ask.volume / 1e8 // amount RDG available for sale via ask
          let requested_vol = total_btc * p // BTC * (RDG/BTC) = vol RDG unit;
          let thisBtc = v / p; // RDG / RDG / BTC = BTC
          console.log(`ask ${ask} p ${p} v ${v} requested_vol ${requested_vol}
          thisBtc ${thisBtc} total_btc ${total_btc} total_fulfilled ${total_fulfilled} float_btc_value ${floatBtcValue}`)
          if (requested_vol > v) {
            total_btc -= thisBtc;
            total_fulfilled += v
          } else {
            total_btc = 0;
            total_fulfilled += requested_vol
            break
          }
        }
        this.rdg_buy_amount = total_fulfilled;
      }

      // If the value is updated by the other watcher, do not recompute
      if (this.updatingValue) return;

      this.updatingValue = true;
      if (!isNaN(floatBtcValue)) {
        // console.log("New usd value " + floatBtcValue)
        this.inputUSD = floatBtcValue * this.usdBtcRate;
      }
      this.updatingValue = false;
    }
  },
  computed: {

    publicKeyCompact() {
      let excludePrefixes = ['0a220a20', '0a230a2103', '0a230a2102']
      let dat = this.hashData.address_pool_info.public_key;

        for (let pfx of excludePrefixes) {
          if (dat.startsWith(pfx)) {
            return dat.substring(pfx.length);
          }
        }
      return dat
    },
    btcExplorerLink() {

      var net = "testnet/";
      let btcAddress = this.hashData.address_pool_info.addresses['Bitcoin'];

      if (!btcAddress.startsWith("tb")) {
        net = "";
      }

      return "https://blockstream.info/" + net + "address/" + btcAddress;
    },
    ethExplorerLink() {
      let btcAddress = this.hashData.address_pool_info.addresses['Bitcoin'];
      let ethAddress = this.hashData.address_pool_info.addresses['Ethereum'];
      var retUrl = "https://sepolia.etherscan.io/address/" + ethAddress;
      if (!btcAddress.startsWith("tb")) {
        retUrl = "https://etherscan.io/address/" + ethAddress;
      }
      return retUrl;
    },

    usdBtcRate() {
      return this.$store.state.btcExchangeRate;
    },
    usdEthRate() {
      return this.$store.state.ethExchangeRate;
    },
    askPriceUsdRdg() {
      return '100.00';
    },
    bidPriceUsdRdg() {
      return '100.00';
    },
  },
  props: {
    hashDataInitial: Object,
  },
  methods: {
    // Now you can use toTitleCase in this component
    formatTitle(key) {
      return toTitleCase(key);
    },
    shortenExeChecksum(h) {
      return h.substring(h.length - 8)
    }
  },
}

</script>

<style>


.radio-option {
  margin-right: 20px;
}

.radio-option input[type="radio"] {
  /* display: none; */
}

.radio-option span {
  padding: 10px;
  border: 1px solid #ccc;
  display: inline-block;
  margin-right: 5px;
  cursor: pointer;
}

.radio-option input[type="radio"]:checked + span {
  /* background-color: #ddd; */
}

.radio-holder {
  padding-left: 10px;
  background-color: #191a19 !important;
}

.grid-container {
  display: grid;
  grid-template-columns: 1fr 6fr; /* Adjust as needed */
  gap: 10px; /* Adjust as needed */
  padding-top: 5px;
  padding-bottom: 5px;
}

.hash-container {
  display: flex;
  align-items: center;
}
.detail-group {
  padding-top: 15px;
  padding-bottom: 15px;
  padding-left: 10px;
  background-color: #191a19 !important;
}
.signature {
  word-break: break-word;
  overflow-wrap: break-word;
}

.grid-container {
  display: grid;
  grid-template-columns: 1fr 6fr; /* Adjust as needed */
  gap: 10px; /* Adjust as needed */
  padding-top: 5px;
  padding-bottom: 5px;
}

.hash-container {
  display: flex;
  align-items: center;
}

.flex-center {
  display: flex;
  align-items: center;
}


.detail-group {
  padding-top: 15px;
  padding-bottom: 15px;
  padding-left: 10px;
  padding-right: 20px;
  background-color: #191a19 !important;
}
.pagination {
  background-color: #000000; /* slightly lighter grey for the active page */
}

.page-link {
  color: #fff; /* white text */
  background-color: #000000; /* slightly lighter grey for the active page */
}

.page-item.active .page-link {
  background-color: #000000; /* slightly lighter grey for the active page */
  border-color: #666;
}

.page-item.disabled .page-link {
  color: #999; /* lighter grey text for disabled buttons */
}

.chart-container {
  /* position: relative; Important for responsive sizing */
  height: 600px;
  width: 600px;
  color: #FFFFFF;
}

label {
  margin-right: 20px; /* Adjust the value as per your requirement */
}


.search-bar {
  background-color: #000;
}

.search-input,
.search-input:focus {
  box-sizing: border-box;
  min-width: 200px;
  max-width: 200px;
  background-color: #191a19;
  color: #fff;
}
.search-input::placeholder {
  color: #ccc;
}

/* This will space out each <label> element */
label {
  /* display: block; Makes labels appear on new lines */
  margin-bottom: 10px; /* Adjust this value as per your preference */
}

/* This gives space below your headers and results */
h6, .detail-group, div {
  margin-bottom: 10px; /* Adjust this value as per your preference */
}


.signature {
  word-break: break-word;
  overflow-wrap: break-word;
}

</style>
