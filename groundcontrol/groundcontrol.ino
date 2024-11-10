#include <Servo.h>

#define SERVO_PIN_FUEL 2       // Digital pin 2 for the fuel valve servo
#define SERVO_PIN_OXI 3        // Digital pin 3 for the oxidizer valve servo
#define FLOW_SENSOR_PIN_FUEL 4 // Digital pin 4 for the fuel flow sensor
#define FLOW_SENSOR_PIN_OXI 5  // Digital pin 5 for the oxidizer flow sensor

// Valve control variables
const int POS_OPEN = 115;
const int POS_CLOSE = 180;
int desiredPositionFuel = POS_CLOSE;
int desiredPositionOxi = POS_CLOSE;
Servo valveServoFuel;
Servo valveServoOxi;

// Flow sensor variables
volatile int pulseCountFuel = 0;
volatile int pulseCountOxi = 0;
float flowRateFuel = 0.0;
float flowRateOxi = 0.0;
unsigned long previousMillis =
    0; // Track the last time we updated the flow rate
const unsigned long interval =
    100; // Shorter interval (100 ms) for more frequent updates

// Serial communication variables
unsigned long lastSerialTime = 0;
const unsigned long SERIAL_TIMEOUT =
    1000; // Close valves if no serial data for 1 second
unsigned long currentMillis = millis();

void setup() {
  // Set baud rate to 115200
  Serial.begin(115200);

  // Initialize flow sensor pin
  pinMode(FLOW_SENSOR_PIN_FUEL, INPUT_PULLUP);
  pinMode(FLOW_SENSOR_PIN_OXI, INPUT_PULLUP);

  // Interrupts for pulse counting
  attachInterrupt(digitalPinToInterrupt(FLOW_SENSOR_PIN_FUEL), countPulseFuel,
                  FALLING);
  attachInterrupt(digitalPinToInterrupt(FLOW_SENSOR_PIN_OXI), countPulseOxi,
                  FALLING);

  // Initialize servo pins
  valveServoFuel.attach(2);
  valveServoOxi.attach(3);

  // Initialize to safe position
  valveServoFuel.write(POS_CLOSE);
  valveServoOxi.write(POS_CLOSE);
}

// We print a csv to serial, so we can read it in the ground control software
// The ground control software will then plot the data and show it to the user
// The format is as follows:
// time, flow rate fuel, flow rate oxi, pulse count fuel, pulse count oxi,
// desired position fuel, desired position oxi, is_emergency
void loop() {
  currentMillis = millis();

  // Check for serial timeout safety
  if (isEmergency()) {
    // Communication lost - emergency close
    desiredPositionFuel = POS_CLOSE;
    desiredPositionOxi = POS_CLOSE;
    valveServoFuel.write(POS_CLOSE);
    valveServoOxi.write(POS_CLOSE);
  }

  // Calculate flow rate every 100 ms
  if (currentMillis - previousMillis >= interval) {
    previousMillis = currentMillis;

    // Calculate flow rate in L/min using Q = F / 7.5
    // Convert pulse count in 100 ms to frequency (Hz)
    // Scale up by 10 to get pulses per second
    // Flow rate in L/min
    flowRateFuel = (pulseCountFuel * 10.0) / 7.5;
    flowRateOxi = (pulseCountOxi * 10.0) / 7.5;

    // Print CSV format data
    Serial.print(currentMillis);
    Serial.print(",");
    Serial.print(flowRateFuel);
    Serial.print(",");
    Serial.print(flowRateOxi);
    Serial.print(",");
    Serial.print(pulseCountFuel);
    Serial.print(",");
    Serial.print(pulseCountOxi);
    Serial.print(",");
    Serial.print(desiredPositionFuel);
    Serial.print(",");
    Serial.print(desiredPositionOxi);
    Serial.print(",");
    Serial.println(isEmergency() ? 1 : 0);

    // Reset pulse count after each calculation
    pulseCountFuel = 0;
    pulseCountOxi = 0;
  }

  // Check for servo control commands
  // Process commands from serial input
  // Format: "fuel,oxi" where each value is 1 (open) or 0 (close)
  // Example: "1,0" opens fuel valve and closes oxidizer valve
  if (Serial.available() > 0) {
    // Read the command and update communication time
    String command = Serial.readStringUntil('\n');
    lastSerialTime = currentMillis;

    // Parse the two valve commands
    int commaIndex = command.indexOf(',');
    if (commaIndex != -1) {
      int fuelCommand = command.substring(0, commaIndex).toInt();
      int oxiCommand = command.substring(commaIndex + 1).toInt();

      desiredPositionFuel = fuelCommand ? POS_OPEN : POS_CLOSE;
      desiredPositionOxi = oxiCommand ? POS_OPEN : POS_CLOSE;
      valveServoFuel.write(desiredPositionFuel);
      valveServoOxi.write(desiredPositionOxi);
    }

    // Clear the serial buffer
    while (Serial.available() > 0) {
      Serial.read();
    }
  }
}

// Interrupt Service Routine to count pulses
void countPulseFuel() { pulseCountFuel++; }
void countPulseOxi() { pulseCountOxi++; }

bool isEmergency() { return currentMillis - lastSerialTime > SERIAL_TIMEOUT; }
