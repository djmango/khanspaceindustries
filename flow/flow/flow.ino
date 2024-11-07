#define FLOW_SENSOR_PIN 2  // Digital pin 2 for the flow sensor
volatile int pulseCount = 0;
float flowRate = 0.0;
unsigned long previousMillis = 0; // Track the last time we updated the flow rate
const unsigned long interval = 100;  // Shorter interval (100 ms) for more frequent updates

void setup() {
  Serial.begin(115200);  // Set baud rate to 115200
  pinMode(FLOW_SENSOR_PIN, INPUT_PULLUP);  // Initialize flow sensor pin
  attachInterrupt(digitalPinToInterrupt(FLOW_SENSOR_PIN), countPulse, FALLING);  // Interrupt for pulse counting
}

void loop() {
  unsigned long currentMillis = millis();

  // Calculate flow rate every 100 ms
  if (currentMillis - previousMillis >= interval) {
    previousMillis = currentMillis;

    // Convert pulse count in 100 ms to frequency (Hz)
    float frequency = (pulseCount * 10.0); // Scale up by 10 to get pulses per second

    // Calculate flow rate in L/min using Q = F / 7.5
    flowRate = frequency / 7.5;  // Flow rate in L/min

    pulseCount = 0;  // Reset pulse count after each calculation

    // Print flow rate data
    Serial.println(flowRate);
  }
}

// Interrupt Service Routine to count pulses
void countPulse() {
  pulseCount++;
}
