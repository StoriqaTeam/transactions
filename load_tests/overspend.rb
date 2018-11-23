#! /usr/local/bin/ruby

require 'yaml'
require 'securerandom'
require 'json'
require 'net/http'
require 'uri'

$http = Net::HTTP.new('localhost', 8010)

def create_withraw_tx(config, currency)
  {
    id: SecureRandom::uuid,
    userId: config['userId'],
    from: config['account_id'][currency],
    to: config['to_address'],
    toType: "address",
    toCurrency: currency,
    value: config['value'][currency],
    valueCurrency: currency,
    fee: 0,
  }
end

def post_tx(config, payload)
  token = config["token"]
  headers = {
    'Content-Type': 'application/json',
    'Authorization': "Bearer #{token}",
  }
  request = Net::HTTP::Post.new(config['url'], headers)
  request.body = payload.to_json
  $http.request(request)
end


config = YAML.load_file('overspend.yml')
threads = []

config["number_of_threads"].times do
  threads << Thread::new do
    payload = create_withraw_tx(config, "stq")
    resp = post_tx(config, payload)
    puts "Code: #{resp.code}, Body: #{resp.body}"
  end
end

threads.each { |thr| thr.join }
