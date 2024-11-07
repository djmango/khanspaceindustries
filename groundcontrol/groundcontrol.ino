#include <Servo.h>

// Valve control variables
Servo valveServo1;
Servo valveServo2;
const int MIN_POS = 115;
const int MAX_POS = 180;
const int CLOSE_POS = 180;
int desiredPosition = MIN_POS;

// Flow sensor variables
#define FLOW_SENSOR_PIN 2  // Digital pin 2 for the flow sensor
volatile int pulseCount = 0;
float flowRate = 0.0;
unsigned long previousMillis = 0; // Track the last time we updated the flow rate
const unsigned long interval = 100;  // Shorter interval (100 ms) for more frequent updates

void setup() {
  Serial.begin(115200);  // Set baud rate to 115200
  pinMode(FLOW_SENSOR_PIN, INPUT_PULLUP);  // Initialize flow sensor pin
  attachInterrupt(digitalPinToInterrupt(FLOW_SENSOR_PIN), countPulse, FALLING);  // Interrupt for pulse counting

  valveServo1.attach(2);
  valveServo2.attach(3);
  valveServo1.write(MIN_POS); // Initialize to safe position
  valveServo2.write(MIN_POS); // Initialize to safe position
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

  // Serial.println("Enter position for both servos: ");
  // while (Serial.available() == 0) {};
  // desiredPosition = Serial.readString().toInt();
  // 
  // // Safety checks before moving the servos
  // if (isSafeToMove()) {
  //   int constrainedPosition = constrain(desiredPosition, MIN_POS, MAX_POS);
  //   valveServo1.write(constrainedPosition);
  //   valveServo2.write(constrainedPosition);
  //   Serial.print("Both valve servos turned to: ");
  //   Serial.println(constrainedPosition);
  // } else {
  //   // Take appropriate action (e.g., shut down)
  //   emergencyShutdown();
  // }

  // delay(1000);
}

// Interrupt Service Routine to count pulses
void countPulse() {
  pulseCount++;
}

// Valve safety logic
bool isSafeToMove() {
  return true; // Placeholder
}

void emergencyShutdown() {
  valveServo1.write(CLOSE_POS); // Move to safe position
  valveServo2.write(CLOSE_POS); // Move to safe position
  Serial.println("Emergency shutdown initiated!");
  // Additional shutdown procedures
}
